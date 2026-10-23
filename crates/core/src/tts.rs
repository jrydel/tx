//! The narrator's voice: text to speech with Piper (rhasspy/piper, neural, runs on this computer).
//! The Piper release and the voice models are downloaded into the data dir on first use.

use crate::paths;
use anyhow::{Context, Result, anyhow, bail, ensure};
use futures_util::StreamExt;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, Semaphore};

/// Longest text one call narrates.
pub const MAX_CHARS: usize = 400;

/// The release bundles the binary, its libraries and espeak-ng-data under `piper/`.
const PIPER_RELEASE: &str = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2";
const PIPER_ASSET: &str = if cfg!(windows) {
    "piper_windows_amd64.zip"
} else if cfg!(target_os = "macos") {
    if cfg!(target_arch = "aarch64") {
        "piper_macos_aarch64.tar.gz"
    } else {
        "piper_macos_x64.tar.gz"
    }
} else if cfg!(target_arch = "aarch64") {
    "piper_linux_aarch64.tar.gz"
} else {
    "piper_linux_x86_64.tar.gz"
};
const VOICES: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main";

/// One download at a time, so parallel first calls don't fetch everything twice.
static SETUP: Mutex<()> = Mutex::const_new(());
/// Piper processes running at once.
static PIPER: Semaphore = Semaphore::const_new(2);

pub struct Narration {
    /// A complete WAV file.
    pub wav: Vec<u8>,
    pub ms: u64,
}

/// Voice path in the piper-voices repository, without extension.
fn voice(lang: &str) -> Result<&'static str> {
    match lang {
        "en" => Ok("en/en_US/lessac/medium/en_US-lessac-medium"),
        "cs" => Ok("cs/cs_CZ/jirka/medium/cs_CZ-jirka-medium"),
        other => bail!("no narrator voice for language {other:?} (en or cs)"),
    }
}

/// Speak `text` (at most `MAX_CHARS`) in `lang` ("en" or "cs"). Spoken lines are cached on disk.
pub async fn narrate(text: &str, lang: &str) -> Result<Narration> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    ensure!(!text.is_empty(), "nothing to narrate");
    ensure!(
        text.chars().count() <= MAX_CHARS,
        "text is longer than {MAX_CHARS} characters"
    );
    let voice = voice(lang)?;
    let name = voice.rsplit('/').next().unwrap_or(voice);
    let cache = paths::cache_dir().join("tts");
    let wav_path = cache.join(format!(
        "{}.wav",
        sha1_smol::Sha1::from(format!("{name}\n{text}")).digest()
    ));
    if let Ok(wav) = tokio::fs::read(&wav_path).await
        && let Ok(ms) = wav_ms(&wav)
    {
        return Ok(Narration { wav, ms });
    }

    let (piper, model) = setup(voice, name)
        .await
        .map_err(|e| anyhow!("Couldn't set up the narrator voice: {e:#}"))?;
    let _slot = PIPER.acquire().await?;
    tokio::fs::create_dir_all(&cache).await?;
    let mut part = wav_path.clone().into_os_string();
    part.push(".part");
    let mut child = paths::command(&piper)
        .arg("--model")
        .arg(&model)
        .arg("--output_file")
        .arg(&part)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("Couldn't start the narrator voice")?;
    let mut stdin = child.stdin.take().context("no stdin")?;
    stdin.write_all(text.as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    drop(stdin);
    let out = child.wait_with_output().await?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let why = stderr.lines().rev().find(|l| !l.trim().is_empty());
        bail!(
            "Couldn't synthesize the narration: {}",
            why.unwrap_or("the voice engine failed")
        );
    }
    let wav = tokio::fs::read(&part).await?;
    let ms = wav_ms(&wav).map_err(|e| anyhow!("Couldn't synthesize the narration: {e:#}"))?;
    tokio::fs::rename(&part, &wav_path).await?;
    Ok(Narration { wav, ms })
}

/// Piper and the voice's model files, downloaded when missing.
async fn setup(voice: &str, name: &str) -> Result<(PathBuf, PathBuf)> {
    let dir = paths::data_dir().join("tts");
    let piper = dir
        .join("piper")
        .join(format!("piper{}", std::env::consts::EXE_SUFFIX));
    let model = dir.join("voices").join(format!("{name}.onnx"));
    if piper.is_file() && model.is_file() {
        return Ok((piper, model));
    }
    let _one = SETUP.lock().await;
    if !piper.is_file() {
        install_piper(&dir).await.context("downloading Piper")?;
    }
    // The config first: the model file appearing means both are complete.
    for ext in ["onnx.json", "onnx"] {
        let file = dir.join("voices").join(format!("{name}.{ext}"));
        if !file.is_file() {
            download(&format!("{VOICES}/{voice}.{ext}"), &file)
                .await
                .with_context(|| format!("downloading the {name} voice"))?;
        }
    }
    Ok((piper, model))
}

async fn install_piper(dir: &Path) -> Result<()> {
    let archive = dir.join(PIPER_ASSET);
    download(&format!("{PIPER_RELEASE}/{PIPER_ASSET}"), &archive).await?;
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let staging = tempfile::tempdir_in(&dir)?;
        let file = std::fs::File::open(&archive)?;
        if PIPER_ASSET.ends_with(".zip") {
            zip::ZipArchive::new(file)?.extract(staging.path())?;
        } else {
            tar::Archive::new(flate2::read::GzDecoder::new(file)).unpack(staging.path())?;
        }
        let dest = dir.join("piper");
        let _ = std::fs::remove_dir_all(&dest); // an earlier attempt that died half way
        std::fs::rename(staging.path().join("piper"), dest)?;
        std::fs::remove_file(&archive)?;
        anyhow::Ok(())
    })
    .await?
}

/// Download `url` to `dest` through a `.part` file, so `dest` only ever exists complete.
async fn download(url: &str, dest: &Path) -> Result<()> {
    log::info!("downloading {url}");
    if let Some(dir) = dest.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let resp = reqwest::Client::builder()
        .user_agent("tx")
        .build()?
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    let mut part: OsString = dest.as_os_str().to_owned();
    part.push(".part");
    let mut out = tokio::fs::File::create(&part).await?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        out.write_all(&chunk?).await?;
    }
    out.flush().await?;
    tokio::fs::rename(&part, dest).await?;
    Ok(())
}

/// Length of a WAV file in milliseconds, from its `fmt ` and `data` chunks.
pub fn wav_ms(wav: &[u8]) -> Result<u64> {
    ensure!(
        wav.len() >= 12 && &wav[..4] == b"RIFF" && &wav[8..12] == b"WAVE",
        "not a WAV file"
    );
    let le32 = |at: usize| u32::from_le_bytes([wav[at], wav[at + 1], wav[at + 2], wav[at + 3]]);
    let (mut pos, mut byte_rate) = (12, 0);
    while pos + 8 <= wav.len() {
        let (id, size, body) = (&wav[pos..pos + 4], le32(pos + 4) as usize, pos + 8);
        match id {
            b"fmt " => {
                ensure!(wav.len() >= body + 12, "truncated WAV header");
                byte_rate = le32(body + 8);
            }
            b"data" => {
                ensure!(byte_rate > 0, "WAV data without a format");
                // A streamed file can claim more (or 0xFFFFFFFF) than it holds.
                let len = size.min(wav.len() - body);
                return Ok(len as u64 * 1000 / u64::from(byte_rate));
            }
            _ => {}
        }
        pos = body + size + (size & 1);
    }
    bail!("no audio in the WAV file")
}

#[cfg(test)]
mod tests {
    use super::wav_ms;

    fn wav(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut body = b"WAVE".to_vec();
        for (id, data) in chunks {
            body.extend(*id);
            body.extend((data.len() as u32).to_le_bytes());
            body.extend(data);
            if data.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut out = b"RIFF".to_vec();
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(body);
        out
    }

    fn fmt(byte_rate: u32) -> Vec<u8> {
        let mut f = vec![1, 0, 1, 0]; // PCM, mono
        f.extend(22_050u32.to_le_bytes());
        f.extend(byte_rate.to_le_bytes());
        f.extend([2, 0, 16, 0]);
        f
    }

    #[test]
    fn wav_duration_from_data_chunk() {
        // 1.5 s of 16-bit mono at 22.05 kHz, with a LIST chunk (odd length) in between.
        let w = wav(&[
            (b"fmt ", fmt(44_100)),
            (b"LIST", vec![0; 5]),
            (b"data", vec![0; 66_150]),
        ]);
        assert_eq!(wav_ms(&w).unwrap(), 1500);
    }

    #[test]
    fn wav_duration_clamps_a_data_size_that_overshoots() {
        let mut w = wav(&[(b"fmt ", fmt(44_100)), (b"data", vec![0; 44_100])]);
        let size_at = w.len() - 44_100 - 4;
        w[size_at..size_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(wav_ms(&w).unwrap(), 1000);
    }

    #[test]
    fn wav_duration_rejects_other_files() {
        assert!(wav_ms(b"ID3 not audio at all").is_err());
        assert!(wav_ms(&wav(&[(b"data", vec![0; 10])])).is_err());
    }
}
