//! Transcripts via the bundled yt-dlp binary: platform captions first, local Whisper otherwise.

use crate::captions::{Seg, Transcript, parse_json3, parse_vtt};
use crate::paths;
use anyhow::{Context, Result, anyhow, bail, ensure};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

/// A tool tx needs: the bundled/installed copy, or (standalone `tx` CLI) the official build,
/// downloaded into the data dir on first use.
async fn tool(name: &str) -> Result<PathBuf> {
    if let Some(p) = paths::bundled(name) {
        return Ok(p);
    }
    let url = match name {
        "yt-dlp" => {
            format!("https://github.com/yt-dlp/yt-dlp/releases/latest/download/{YT_DLP_ASSET}")
        }
        "qjs" => format!(
            "https://github.com/quickjs-ng/quickjs/releases/download/{QUICKJS_VERSION}/{QJS_ASSET}"
        ),
        other => bail!("unknown tool: {other}"),
    };
    log::info!("downloading {name} from {url}");
    install_bin(name, &url)
        .await
        .with_context(|| format!("downloading {name}"))
}

/// Download an executable into the data dir's bin/, where `paths::bundled` looks first.
async fn install_bin(name: &str, url: &str) -> Result<PathBuf> {
    let bytes = reqwest::Client::new()
        .get(url)
        .header("User-Agent", "tx")
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let dir = paths::data_dir().join("bin");
    tokio::fs::create_dir_all(&dir).await?;
    let dest = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let part = dest.with_extension("part");
    tokio::fs::write(&part, &bytes).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755)).await?;
    }
    tokio::fs::rename(&part, &dest).await?; // atomic on Unix; fails harmlessly on Windows while the tool runs
    Ok(dest)
}

/// Common flags. YouTube needs a JS runtime for signature solving: tx uses QuickJS.
async fn base_args() -> Vec<OsString> {
    let mut args: Vec<OsString> = ["--no-warnings", "--ignore-config"]
        .map(OsString::from)
        .into();
    match tool("qjs").await {
        Ok(qjs) => args.extend([
            "--no-js-runtimes".into(),
            "--js-runtimes".into(),
            format!("quickjs:{}", qjs.display()).into(),
        ]),
        Err(e) => log::warn!("no QuickJS ({e:#}); yt-dlp falls back to any installed JS runtime"),
    }
    args
}

/// Run yt-dlp; return stdout, or the ERROR line yt-dlp printed.
pub async fn run(args: Vec<OsString>) -> Result<Vec<u8>> {
    let out = paths::command(&tool("yt-dlp").await?)
        .args(base_args().await)
        .args(args)
        .output()
        .await
        .context("running yt-dlp")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let msg = stderr
            .lines()
            .rev()
            .find(|l| l.starts_with("ERROR"))
            .unwrap_or(stderr.trim());
        bail!("{}", msg.trim_start_matches("ERROR: "));
    }
    Ok(out.stdout)
}

pub async fn json(args: Vec<OsString>) -> Result<Value> {
    Ok(serde_json::from_slice(&run(args).await?)?)
}

/// `--cookies-from-browser` spec: "chrome", "firefox", "chrome:Profile 2", …
pub fn cookies_args(browser: &str) -> Vec<OsString> {
    vec!["--cookies-from-browser".into(), browser.into()]
}

pub fn pick_track<'a>(keys: &[&'a str], prefs: &[String]) -> Option<&'a str> {
    let keys: Vec<&str> = keys.iter().copied().filter(|k| *k != "live_chat").collect();
    for p in prefs {
        for want in [format!("{p}-orig"), p.clone()] {
            if let Some(k) = keys.iter().find(|k| **k == want) {
                return Some(k);
            }
        }
    }
    for p in prefs {
        if let Some(k) = keys
            .iter()
            .find(|k| k.split('-').next() == Some(p.as_str()))
        {
            return Some(k);
        }
    }
    keys.iter().find(|k| k.ends_with("-orig")).copied()
}

async fn captions(info: &Value, lang: Option<&str>) -> Result<Option<(Vec<Seg>, String)>> {
    let prefs: Vec<String> = match lang {
        Some(l) => vec![l.to_string()],
        None => [info["language"].as_str(), Some("en")]
            .into_iter()
            .flatten()
            .map(String::from)
            .collect(),
    };
    for (field, kind, any_fallback) in [
        ("subtitles", "manual", lang.is_none()),
        ("automatic_captions", "auto", false),
    ] {
        let Some(tracks) = info[field].as_object() else {
            continue;
        };
        let keys: Vec<&str> = tracks.keys().map(String::as_str).collect();
        let key = pick_track(&keys, &prefs).or_else(|| {
            any_fallback
                .then(|| keys.iter().copied().find(|k| *k != "live_chat"))
                .flatten()
        });
        let Some(key) = key else { continue };
        let formats = tracks[key].as_array().cloned().unwrap_or_default();
        for ext in ["json3", "vtt", "srt"] {
            let Some(url) = formats
                .iter()
                .find(|f| f["ext"] == ext)
                .and_then(|f| f["url"].as_str())
            else {
                continue;
            };
            let body = reqwest::get(url).await?.error_for_status()?.text().await?;
            let segs = if ext == "json3" {
                parse_json3(&serde_json::from_str(&body)?)
            } else {
                parse_vtt(&body)
            };
            if !segs.is_empty() {
                return Ok(Some((segs, format!("captions {key} ({kind})"))));
            }
        }
    }
    Ok(None)
}

/// Progress callback for long local transcriptions: (seconds done, total seconds).
pub type Progress = Box<dyn Fn(&str) + Send + Sync>;

pub async fn fetch(
    url: &str,
    lang: Option<&str>,
    force_whisper: bool,
    model: &str,
    progress: &Progress,
) -> Result<Transcript> {
    let mut info = json(vec![
        "-J".into(),
        "--skip-download".into(),
        "--no-playlist".into(),
        url.into(),
    ])
    .await?;
    let mut item = None;
    if info["_type"] == "playlist" {
        // e.g. an X post with several media items
        let entries = info["entries"]
            .as_array()
            .filter(|e| !e.is_empty())
            .ok_or_else(|| anyhow!("no video found at URL"))?;
        if entries.len() > 1 {
            progress(&format!("{} videos at URL; using the first", entries.len()));
        }
        info = entries[0].clone();
        item = Some(info["playlist_index"].as_u64().unwrap_or(1));
    }
    let got = if force_whisper {
        None
    } else {
        captions(&info, lang).await?
    };
    let (segs, source) = match got {
        Some(g) => g,
        None => {
            if !force_whisper {
                progress("no captions found; transcribing the audio locally");
            }
            crate::whisper::transcribe(url, item, info["duration"].as_f64(), lang, model, progress)
                .await?
        }
    };
    Ok(Transcript {
        title: info["title"].as_str().unwrap_or(url).to_string(),
        url: url.to_string(),
        uploader: info["uploader"]
            .as_str()
            .or(info["channel"].as_str())
            .map(String::from),
        duration: info["duration"].as_f64(),
        source,
        segs,
    })
}

/// Where `load` caches the transcript of `url` for these options.
pub fn cache_path(url: &str, lang: Option<&str>, whisper: bool, model: &str) -> PathBuf {
    let mode = if whisper {
        format!("whisper:{model}")
    } else {
        "auto".into()
    };
    let key = sha1_smol::Sha1::from(format!("{url}|{}|{mode}", lang.unwrap_or("None")))
        .digest()
        .to_string();
    paths::cache_dir().join(format!("{}.json", &key[..16]))
}

/// Cached transcript for `url`; returns it with its cache path (.json).
pub async fn load(
    url: &str,
    lang: Option<&str>,
    whisper: bool,
    model: &str,
    refresh: bool,
    progress: &Progress,
) -> Result<(Transcript, PathBuf)> {
    let path = cache_path(url, lang, whisper, model);
    if !refresh
        && let Ok(bytes) = tokio::fs::read(&path).await
        && let Ok(t) = serde_json::from_slice(&bytes)
    {
        return Ok((t, path));
    }
    let t = fetch(url, lang, whisper, model, progress).await?;
    tokio::fs::write(&path, serde_json::to_vec(&t)?).await?;
    Ok((t, path))
}

/// A direct URL of a YouTube video's file on Google's CDN, with picture and sound in one file
/// (for a `<video>` element).
#[derive(Clone)]
pub struct Stream {
    pub url: String,
    pub mime: &'static str,
    pub height: u64,
    pub expires_at: DateTime<Utc>,
}

/// Progressive formats only: 18 (360p MP4), else the best that has both picture and sound.
const STREAM_FORMAT: &str =
    "18/best[vcodec!=none][acodec!=none][ext=mp4]/best[vcodec!=none][acodec!=none]";
/// A cached URL is replaced this long before it expires, so a video that starts now plays through.
const STREAM_MARGIN: TimeDelta = TimeDelta::minutes(30);
static STREAMS: LazyLock<Mutex<HashMap<String, Stream>>> = LazyLock::new(Default::default);

/// Playable URL for a YouTube video id, from yt-dlp; kept until shortly before it expires.
pub async fn stream(video_id: &str) -> Result<Stream> {
    ensure!(
        !video_id.is_empty()
            && video_id.len() <= 32
            && video_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        "not a YouTube video id: {video_id:?}"
    );
    let fresh = |s: &Stream| s.expires_at - STREAM_MARGIN > Utc::now();
    if let Some(s) = STREAMS.lock().unwrap().get(video_id).filter(|s| fresh(s)) {
        return Ok(s.clone());
    }
    let out = run(vec![
        "--no-playlist".into(),
        "-f".into(),
        STREAM_FORMAT.into(),
        "--print".into(),
        "%(url)s\t%(height)s\t%(ext)s".into(),
        format!("https://www.youtube.com/watch?v={video_id}").into(),
    ])
    .await?;
    let out = String::from_utf8_lossy(&out);
    let mut cols = out.lines().next().unwrap_or_default().split('\t');
    let url = cols
        .next()
        .filter(|u| u.starts_with("https://"))
        .context("yt-dlp found no playable stream")?
        .to_string();
    let height = cols.next().and_then(|h| h.parse().ok()).unwrap_or(0);
    let mime = if cols.next() == Some("webm") {
        "video/webm"
    } else {
        "video/mp4"
    };
    let expires_at = expires_at(&url).context("the stream URL has no expiry")?;
    let stream = Stream {
        url,
        mime,
        height,
        expires_at,
    };
    let mut streams = STREAMS.lock().unwrap();
    streams.retain(|_, s| fresh(s));
    streams.insert(video_id.to_string(), stream.clone());
    Ok(stream)
}

/// The `expire=` (Unix seconds) query parameter of a Google video URL.
pub fn expires_at(url: &str) -> Option<DateTime<Utc>> {
    let secs = url
        .split_once('?')?
        .1
        .split('&')
        .find_map(|kv| kv.strip_prefix("expire="))?;
    DateTime::from_timestamp(secs.parse().ok()?, 0)
}

/// Official standalone builds for this platform, as named in the projects' GitHub releases.
const YT_DLP_ASSET: &str = if cfg!(windows) {
    "yt-dlp.exe"
} else if cfg!(target_os = "macos") {
    "yt-dlp_macos"
} else if cfg!(target_arch = "aarch64") {
    "yt-dlp_linux_aarch64"
} else {
    "yt-dlp_linux"
};
const QUICKJS_VERSION: &str = "v0.17.0"; // keep in step with scripts/sidecars.ts
const QJS_ASSET: &str = if cfg!(windows) {
    "qjs-windows-x86_64.exe"
} else if cfg!(target_os = "macos") {
    if cfg!(target_arch = "aarch64") {
        "qjs-darwin-arm64"
    } else {
        "qjs-darwin-x86_64"
    }
} else if cfg!(target_arch = "aarch64") {
    "qjs-linux-aarch64"
} else {
    "qjs-linux-x86_64"
};

/// YouTube changes often break old yt-dlp releases, so a long-running watcher keeps its own
/// current copy in the data dir (preferred by `paths::bundled`). Returns the new version, if any.
pub async fn keep_current() -> Result<Option<String>> {
    let client = reqwest::Client::new();
    let latest: Value = client
        .get("https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest")
        .header("User-Agent", "tx")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let tag = latest["tag_name"]
        .as_str()
        .context("yt-dlp release has no tag")?;
    let current = String::from_utf8_lossy(&run(vec!["--version".into()]).await?)
        .trim()
        .to_string();
    if current == tag {
        return Ok(None);
    }
    let url = format!("https://github.com/yt-dlp/yt-dlp/releases/download/{tag}/{YT_DLP_ASSET}");
    install_bin("yt-dlp", &url).await?;
    Ok(Some(format!("{current} → {tag}")))
}

#[cfg(test)]
mod tests {
    use super::{expires_at, pick_track};

    #[test]
    fn pick_track_prefers_original_over_translated_and_skips_live_chat() {
        let en = vec!["en".to_string()];
        assert_eq!(
            pick_track(&["live_chat", "de", "en", "en-orig"], &en),
            Some("en-orig")
        );
        assert_eq!(pick_track(&["en-US", "cs"], &en), Some("en-US"));
        assert_eq!(
            pick_track(&["live_chat", "fr-orig", "fr"], &en),
            Some("fr-orig")
        );
        assert_eq!(pick_track(&["live_chat"], &en), None);
    }

    #[test]
    fn stream_expiry_comes_from_the_expire_parameter() {
        let url = "https://rr3---sn-4g5e6nsl.googlevideo.com/videoplayback?expire=1761234567&ei=abc&id=o-1&itag=18";
        assert_eq!(
            expires_at(url).unwrap().to_rfc3339(),
            "2025-10-23T15:49:27+00:00"
        );
        assert_eq!(
            expires_at("https://x.googlevideo.com/videoplayback?ei=a&expire=60")
                .unwrap()
                .timestamp(),
            60
        );
        assert!(expires_at("https://x.googlevideo.com/videoplayback?ei=abc").is_none());
        assert!(expires_at("https://x.googlevideo.com/videoplayback?expire=soon").is_none());
        assert!(expires_at("https://x.googlevideo.com/expire/1761234567/").is_none());
    }
}
