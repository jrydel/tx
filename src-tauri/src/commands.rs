//! The UI's API: Tauri commands over the engine. Errors are returned as display strings.

use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tauri_plugin_autostart::ManagerExt;
use tx_core::captions::paragraphs;
use tx_core::db::{self, Channel, Settings, Video};
use tx_core::engine::{Engine, WorkerState};
use tx_core::{agent, feeds, whisper, ytdlp};

type Res<T> = Result<T, String>;
type Eng<'a> = State<'a, Arc<Engine>>;

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[derive(Serialize)]
pub struct Status {
    #[serde(flatten)]
    worker: WorkerState,
    checked_at: Option<String>,
    watched: i64,
    channels: i64,
    queued: i64,
    agent: bool,
    profile_set: bool,
}

#[tauri::command]
pub fn status(engine: Eng) -> Res<Status> {
    let worker = engine.state.lock().unwrap().clone();
    let conn = engine.db.lock().unwrap();
    let q = |sql: &str| {
        conn.query_row(sql, [], |r| r.get::<_, Option<i64>>(0))
            .map(|v| v.unwrap_or(0))
            .map_err(|e| e.to_string())
    };
    Ok(Status {
        worker,
        checked_at: conn
            .query_row(
                "SELECT MAX(checked_at) FROM channels WHERE enabled = 1",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?,
        watched: q("SELECT COUNT(*) FROM channels WHERE enabled = 1")?,
        channels: q("SELECT COUNT(*) FROM channels")?,
        queued: q("SELECT COUNT(*) FROM videos WHERE status IN ('queued', 'working')")?,
        agent: agent::available(),
        profile_set: !db::get_settings(&conn)
            .map_err(err)?
            .profile
            .trim()
            .is_empty(),
    })
}

#[tauri::command]
pub fn videos(engine: Eng, view: String, channel: Option<String>) -> Res<Vec<Video>> {
    db::videos(&engine.db.lock().unwrap(), &view, channel.as_deref()).map_err(err)
}

#[tauri::command]
pub fn video(engine: Eng, id: String) -> Res<Video> {
    db::video(&engine.db.lock().unwrap(), &id)
        .map_err(err)?
        .ok_or_else(|| "video not found".into())
}

#[derive(Serialize)]
pub struct Paragraph {
    start: f64,
    text: String,
}

#[derive(Serialize)]
pub struct TranscriptView {
    source: String,
    duration: Option<f64>,
    paragraphs: Vec<Paragraph>,
}

#[tauri::command]
pub async fn transcript(engine: Eng<'_>, id: String) -> Res<TranscriptView> {
    let (url, model) = {
        let conn = engine.db.lock().unwrap();
        let v = db::video(&conn, &id)
            .map_err(err)?
            .ok_or("video not found")?;
        (v.url, db::get_settings(&conn).map_err(err)?.model)
    };
    let silent: ytdlp::Progress = Box::new(|_| {});
    let (t, _) = ytdlp::load(&url, None, false, &model, false, &silent)
        .await
        .map_err(err)?;
    let paragraphs = paragraphs(&t.segs, 30.0, 60.0)
        .into_iter()
        .map(|p| Paragraph {
            start: p.start,
            text: p.text,
        })
        .collect();
    Ok(TranscriptView {
        source: t.source,
        duration: t.duration,
        paragraphs,
    })
}

#[tauri::command]
pub fn analyze(engine: Eng, id: String) -> Res<Video> {
    let conn = engine.db.lock().unwrap();
    if !db::requeue(&conn, &id).map_err(err)? {
        return Err("video not found".into());
    }
    engine.wake.notify_one();
    db::video(&conn, &id)
        .map_err(err)?
        .ok_or_else(|| "video not found".into())
}

#[tauri::command]
pub fn check(engine: Eng) {
    engine.wake.notify_one();
}

#[tauri::command]
pub fn channels(engine: Eng) -> Res<Vec<Channel>> {
    db::channels(&engine.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
pub fn set_watching(engine: Eng, id: String, enabled: bool) -> Res<()> {
    if !db::set_enabled(&engine.db.lock().unwrap(), &id, enabled).map_err(err)? {
        return Err("channel not found".into());
    }
    if enabled {
        engine.wake.notify_one(); // fetch its feed now
    }
    Ok(())
}

#[tauri::command]
pub async fn import_subscriptions(engine: Eng<'_>, browser: String) -> Res<usize> {
    let browser = if browser.trim().is_empty() {
        "chrome".to_string()
    } else {
        browser.trim().to_string()
    };
    {
        let conn = engine.db.lock().unwrap();
        let mut s = db::get_settings(&conn).map_err(err)?;
        s.browser = browser.clone();
        db::save_settings(&conn, &s).map_err(err)?;
    }
    feeds::import_subscriptions(&engine.db, &browser)
        .await
        .map_err(|e| format!("Couldn't read YouTube subscriptions from {browser}: {e:#}"))
}

#[tauri::command]
pub async fn add_channel(engine: Eng<'_>, reference: String) -> Res<String> {
    let title = feeds::add_channel(&engine.db, &reference)
        .await
        .map_err(|e| format!("Couldn't find that channel: {e:#}"))?;
    engine.wake.notify_one();
    Ok(title)
}

#[tauri::command]
pub fn settings(engine: Eng) -> Res<Settings> {
    db::get_settings(&engine.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
pub fn save_settings(engine: Eng, settings: Settings) -> Res<Settings> {
    if settings.interval_min < 1 {
        return Err("Check interval must be at least 1 minute.".into());
    }
    if settings.notify_min > 5 {
        return Err("Notification threshold must be 0–5.".into());
    }
    if !whisper::MODELS.iter().any(|(n, _)| *n == settings.model) {
        return Err(format!("Unknown transcription model: {}", settings.model));
    }
    let conn = engine.db.lock().unwrap();
    db::save_settings(&conn, &settings).map_err(err)?;
    db::get_settings(&conn).map_err(err)
}

#[tauri::command]
pub fn models() -> Vec<&'static str> {
    whisper::MODELS.iter().map(|(n, _)| *n).collect()
}

#[tauri::command]
pub fn autostart(app: tauri::AppHandle) -> Res<bool> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Res<bool> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    al.is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_url(app: tauri::AppHandle, url: String) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    if !url.starts_with("https://") {
        return Err("only https links can be opened".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}
