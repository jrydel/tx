//! Local transcription for videos without captions: yt-dlp fetches the audio, symphonia
//! decodes it, whisper.cpp (compiled in via whisper-rs; Metal/Vulkan/CUDA when built with
//! those features) transcribes it.

use crate::captions::{Seg, fmt_time};
use crate::paths;
use crate::ytdlp::{self, Progress};
use anyhow::{Context, Result, anyhow, bail};
use futures_util::StreamExt;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

const SAMPLE_RATE: u32 = 16_000;

/// Settings model name → ggml file published by the whisper.cpp project.
pub const MODELS: &[(&str, &str)] = &[
    ("tiny", "ggml-tiny.bin"),
    ("base", "ggml-base.bin"),
    ("small", "ggml-small.bin"),
    ("medium", "ggml-medium.bin"),
    ("large-v3-turbo", "ggml-large-v3-turbo-q5_0.bin"),
];

async fn model_path(name: &str, progress: &Progress) -> Result<PathBuf> {
    let file = MODELS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, f)| *f)
        .ok_or_else(|| anyhow!("unknown model: {name}"))?;
    let dir = paths::data_dir().join("models");
    let path = dir.join(file);
    if path.is_file() {
        return Ok(path);
    }
    tokio::fs::create_dir_all(&dir).await?;
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file}");
    let resp = reqwest::get(&url).await?.error_for_status()?;
    let total = resp.content_length();
    let part = path.with_extension("part");
    let mut out = tokio::fs::File::create(&part).await?;
    let (mut done, mut last_pct) = (0u64, u64::MAX);
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        out.write_all(&chunk).await?;
        done += chunk.len() as u64;
        if let Some(total) = total {
            let pct = done * 100 / total;
            if pct / 10 != last_pct / 10 {
                progress(&format!("downloading whisper {name} model: {pct}%"));
                last_pct = pct;
            }
        }
    }
    out.flush().await?;
    tokio::fs::rename(&part, &path).await?;
    Ok(path)
}

async fn download_audio(url: &str, item: Option<u64>, dir: &Path) -> Result<PathBuf> {
    // AAC in MP4 first: what symphonia decodes (YouTube's opus/webm is not). X labels its
    // audio-only HLS streams "mp4" with an unknown codec. Smallest muxed video as a last resort,
    // and for a plain audio file (a podcast episode's MP3) its only format.
    let mut args: Vec<OsString> = vec![
        "-q".into(),
        "-f".into(),
        "bestaudio[ext=m4a]/bestaudio[ext=mp4]/bestaudio[acodec^=mp4a]/worst[ext=mp4][acodec!=none]/bestaudio/best".into(),
        "-o".into(),
        dir.join("audio.%(ext)s").into_os_string(),
    ];
    match item {
        Some(i) => args.extend(["--playlist-items".into(), i.to_string().into()]),
        None => args.push("--no-playlist".into()),
    }
    args.push(url.into());
    ytdlp::run(args).await?;
    std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.file_stem().is_some_and(|s| s == "audio"))
        .context("yt-dlp produced no audio file")
}

/// Decode to mono f32 at 16 kHz.
fn decode(path: &Path) -> Result<Vec<f32>> {
    use symphonia::core::{
        audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions,
        io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
    };
    let mss = MediaSourceStream::new(Box::new(std::fs::File::open(path)?), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("unsupported audio container")?;
    let mut format = probed.format;
    // The first track symphonia can decode is the audio one (muxed files also carry video).
    let codecs = symphonia::default::get_codecs();
    let (track_id, mut rate, mut decoder) = format
        .tracks()
        .iter()
        .find_map(|t| {
            codecs
                .make(&t.codec_params, &DecoderOptions::default())
                .ok()
                .map(|d| (t.id, t.codec_params.sample_rate, d))
        })
        .context("unsupported audio codec (tx decodes AAC, MP3 and PCM)")?;
    // Fragmented MP4 (HLS, e.g. X) may not declare a rate up front; decoded frames always do.
    let mut mono = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let buf = match decoder.decode(&packet) {
            Ok(b) => b,
            Err(Error::DecodeError(_)) => continue, // corrupt frame: skip, as players do
            Err(e) => return Err(e.into()),
        };
        let spec = *buf.spec();
        rate = Some(spec.rate);
        let channels = spec.channels.count().max(1);
        let mut samples = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
        samples.copy_interleaved_ref(buf);
        mono.extend(
            samples
                .samples()
                .chunks(channels)
                .map(|f| f.iter().sum::<f32>() / channels as f32),
        );
    }
    Ok(resample(
        &mono,
        rate.context("audio has no decodable frames")?,
        SAMPLE_RATE,
    ))
}

/// Windowed-sinc low-pass at the target Nyquist, then linear interpolation. Plenty for speech.
fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let len = input.len() as i64;
    // Low-pass taps (empty when upsampling: nothing to remove).
    let half = 32i64;
    let taps: Vec<f32> = if from > to {
        let cutoff = 0.45 * to as f32 / from as f32; // cycles per input sample, just under the target Nyquist
        (-half..=half)
            .map(|n| {
                let x = n as f32;
                let sinc = if n == 0 {
                    2.0 * cutoff
                } else {
                    (2.0 * std::f32::consts::PI * cutoff * x).sin() / (std::f32::consts::PI * x)
                };
                let hann = 0.5 + 0.5 * (std::f32::consts::PI * x / (half as f32 + 1.0)).cos();
                sinc * hann
            })
            .collect()
    } else {
        Vec::new()
    };
    // Filtered value at input index j, computed only where an output sample needs it.
    let at = |j: i64| -> f32 {
        if taps.is_empty() {
            return input[j.clamp(0, len - 1) as usize];
        }
        taps.iter()
            .enumerate()
            .map(|(k, t)| input[(j + k as i64 - half).clamp(0, len - 1) as usize] * t)
            .sum()
    };
    let ratio = from as f64 / to as f64;
    let n = (input.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let pos = i as f64 * ratio;
            let j = pos as i64;
            let frac = (pos - j as f64) as f32;
            let a = at(j);
            a + (at(j + 1) - a) * frac
        })
        .collect()
}

pub async fn transcribe(
    url: &str,
    item: Option<u64>,
    duration: Option<f64>,
    lang: Option<&str>,
    model: &str,
    progress: &Progress,
) -> Result<(Vec<Seg>, String)> {
    let model_file = model_path(model, progress).await?;
    let tmp = tempfile::tempdir()?;
    progress("downloading audio…");
    let audio_file = download_audio(url, item, tmp.path()).await?;
    let audio = tokio::task::spawn_blocking(move || decode(&audio_file)).await??;
    if audio.is_empty() {
        bail!("the audio track is empty");
    }
    let decoded = audio.len() as f64 / SAMPLE_RATE as f64;
    if duration.is_some_and(|d| (decoded - d).abs() > d * 0.1 + 5.0) {
        log::warn!(
            "decoded {decoded:.0}s of audio but the video is {:.0}s",
            duration.unwrap_or_default()
        );
    }
    progress(&format!(
        "transcribing {} of audio with whisper {model}…",
        fmt_time(decoded)
    ));
    let lang = lang.map(String::from);
    let model_name = model.to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<i32>();
    let job = tokio::task::spawn_blocking(move || -> Result<(Vec<Seg>, String)> {
        use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
        // whisper.cpp logs to stderr by default; route it through `log` (dropped below Info).
        static HOOKS: std::sync::Once = std::sync::Once::new();
        HOOKS.call_once(whisper_rs::install_logging_hooks);
        let ctx = WhisperContext::new_with_params(&model_file, WhisperContextParameters::default())
            .map_err(|e| anyhow!("loading whisper model: {e}"))?;
        let mut state = ctx.create_state().map_err(|e| anyhow!("{e}"))?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(lang.as_deref().unwrap_or("auto")));
        // ggml's thread pool busy-waits: one worker per SMT sibling made a 40 s clip take 111 s
        // instead of 2.5 s on an 8-core/16-thread CPU. Use about one thread per physical core.
        let logical = std::thread::available_parallelism().map_or(4, |n| n.get());
        params.set_n_threads((logical / 2).clamp(1, 8) as i32);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_progress_callback_safe(move |pct: i32| {
            let _ = tx.send(pct);
        });
        state
            .full(params, &audio)
            .map_err(|e| anyhow!("transcription failed: {e}"))?;
        let segs = (0..state.full_n_segments())
            .filter_map(|i| state.get_segment(i))
            .filter_map(|s| {
                let text = s.to_str_lossy().ok()?.trim().to_string();
                (!text.is_empty()).then(|| Seg {
                    start: s.start_timestamp() as f64 / 100.0,
                    text,
                })
            })
            .collect();
        let detected = whisper_rs::get_lang_str(state.full_lang_id_from_state()).unwrap_or("?");
        Ok((segs, format!("whisper {model_name} ({detected})")))
    });
    // whisper-rs keeps the progress closure (and so the sender) alive, so the channel never
    // closes: forward progress until the job itself finishes.
    let mut job = job;
    let mut last = -1;
    loop {
        tokio::select! {
            result = &mut job => return result?,
            Some(pct) = rx.recv() => {
                if pct / 10 != last / 10 {
                    progress(&format!("transcribing: {pct}%"));
                    last = pct;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resample;

    #[test]
    fn resample_keeps_duration_and_passes_low_tones() {
        let tone: Vec<f32> = (0..48_000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48_000.0).sin())
            .collect();
        let out = resample(&tone, 48_000, 16_000);
        assert_eq!(out.len(), 16_000);
        let rms = (out.iter().map(|x| x * x).sum::<f32>() / out.len() as f32).sqrt();
        assert!(
            (rms - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.05,
            "440 Hz should pass the 7.2 kHz low-pass, rms={rms}"
        );
    }

    #[test]
    fn resample_removes_tones_above_target_nyquist() {
        let tone: Vec<f32> = (0..48_000)
            .map(|i| (2.0 * std::f32::consts::PI * 12_000.0 * i as f32 / 48_000.0).sin())
            .collect();
        let out = resample(&tone, 48_000, 16_000);
        let rms = (out[100..out.len() - 100].iter().map(|x| x * x).sum::<f32>()
            / (out.len() - 200) as f32)
            .sqrt();
        assert!(
            rms < 0.05,
            "12 kHz would alias into the 16 kHz signal, rms={rms}"
        );
    }
}
