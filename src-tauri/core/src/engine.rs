//! The watcher: poll watched channels, analyze new uploads against the profile, notify.

use crate::db::{self, Db, Video};
use crate::{agent, captions, feeds, ytdlp};
use anyhow::Result;
use rusqlite::params;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;

/// What the worker is doing right now; shown in the UI.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorkerState {
    /// idle | checking | analyzing
    pub now: &'static str,
    /// Title of the video being analyzed.
    pub video: Option<String>,
    /// Latest progress message (e.g. "transcribing: 40%").
    pub detail: Option<String>,
}

pub struct Notice {
    pub video_id: String,
    pub title: String,
    pub body: String,
}

pub struct Engine {
    pub db: Db,
    pub wake: Notify,
    pub state: Mutex<WorkerState>,
    pub chats: crate::chat::Chats,
    notify: Box<dyn Fn(Notice) + Send + Sync>,
}

impl Engine {
    pub fn open(notify: impl Fn(Notice) + Send + Sync + 'static) -> Result<Arc<Self>> {
        let conn = db::connect()?;
        // Interrupted by a restart.
        conn.execute(
            "UPDATE videos SET status = 'queued' WHERE status = 'working'",
            [],
        )?;
        conn.execute(
            "UPDATE messages SET pending = 0, error = 'Interrupted' WHERE pending = 1",
            [],
        )?;
        Ok(Arc::new(Self {
            db: Mutex::new(conn),
            wake: Notify::new(),
            state: Mutex::new(WorkerState {
                now: "idle",
                ..Default::default()
            }),
            chats: Default::default(),
            notify: Box::new(notify),
        }))
    }

    fn set_state(&self, now: &'static str, video: Option<String>) {
        *self.state.lock().unwrap() = WorkerState {
            now,
            video,
            detail: None,
        };
    }

    fn progress(self: &Arc<Self>) -> ytdlp::Progress {
        let me = Arc::clone(self);
        Box::new(move |msg: &str| {
            log::info!("{msg}");
            me.state.lock().unwrap().detail = Some(msg.to_string());
        })
    }

    async fn analyze(
        self: &Arc<Self>,
        v: &Video,
        settings: &db::Settings,
    ) -> Result<(agent::Analysis, Option<f64>)> {
        let (t, _) = ytdlp::load(
            &v.url,
            None,
            false,
            &settings.model,
            false,
            &self.progress(),
        )
        .await?;
        self.state.lock().unwrap().detail = Some("asking Claude".into());
        let out = agent::ask(
            &agent::prompt(&settings.profile, &v.title, &v.channel),
            &captions::render(&t, true),
        )
        .await?;
        Ok((agent::parse_analysis(&out), t.duration))
    }

    /// Analyze everything queued (and retryable errors), oldest first. Returns how many finished.
    pub async fn process_queue(self: &Arc<Self>) -> Result<usize> {
        let (pending, settings) = {
            let conn = self.db.lock().unwrap();
            (db::pending(&conn)?, db::get_settings(&conn)?)
        };
        let mut done = 0;
        for v in pending {
            if !db::claim(&self.db.lock().unwrap(), &v.id)? {
                continue;
            }
            self.set_state("analyzing", Some(v.title.clone()));
            let result = self.analyze(&v, &settings).await;
            self.set_state("idle", None);
            let conn = self.db.lock().unwrap();
            match result {
                Err(e) => {
                    log::warn!("analysis failed for {}: {e:#}", v.url);
                    conn.execute(
                        "UPDATE videos SET status = 'error', error = ?1 WHERE id = ?2",
                        params![format!("{e:#}"), v.id],
                    )?;
                }
                Ok((a, duration)) => {
                    conn.execute(
                        "UPDATE videos SET status = 'done', relevance = ?1, verdict = ?2, analysis = ?3, duration = ?4, \
                         error = NULL, analyzed_at = ?5 WHERE id = ?6",
                        params![a.relevance, a.verdict, a.body, duration, db::now(), v.id],
                    )?;
                    done += 1;
                    if a.relevance.is_none_or(|r| r >= settings.notify_min as i64) {
                        let score = a.relevance.map(|r| format!("{r}/5 · ")).unwrap_or_default();
                        (self.notify)(Notice {
                            video_id: v.id.clone(),
                            title: v.title.clone(),
                            body: format!("{score}{}\n{}", v.channel, a.verdict),
                        });
                    }
                }
            }
        }
        Ok(done)
    }

    /// One pass: check feeds, then analyze the queue.
    pub async fn check(self: &Arc<Self>) -> Result<(usize, usize)> {
        self.set_state("checking", None);
        let queued = feeds::poll(&self.db).await;
        self.set_state("idle", None);
        Ok((queued?, self.process_queue().await?))
    }

    /// Check every `interval_min` minutes, or immediately when `wake` is notified.
    /// Once a day, also refresh the bundled yt-dlp.
    pub async fn run_forever(self: Arc<Self>) {
        let mut last_update: Option<std::time::Instant> = None;
        loop {
            if last_update.is_none_or(|t| t.elapsed() > Duration::from_secs(24 * 3600)) {
                last_update = Some(std::time::Instant::now());
                match ytdlp::keep_current().await {
                    Ok(Some(v)) => log::info!("updated yt-dlp {v}"),
                    Ok(None) => {}
                    Err(e) => log::warn!("yt-dlp update check failed: {e:#}"),
                }
            }
            if let Err(e) = self.check().await {
                log::error!("check failed: {e:#}");
                self.set_state("idle", None);
            }
            let minutes = db::get_settings(&self.db.lock().unwrap())
                .map(|s| s.interval_min)
                .unwrap_or(30)
                .max(1);
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(minutes as u64 * 60)) => {}
                _ = self.wake.notified() => {}
            }
        }
    }
}
