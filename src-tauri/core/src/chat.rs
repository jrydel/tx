//! Follow-up questions about a video: one thread per video, answers streamed from the agent.

use crate::db::{self, Message};
use crate::engine::Engine;
use crate::{agent, captions, ytdlp};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::task::AbortHandle;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ChatEvent {
    /// Another piece of the reply being written.
    Delta {
        video_id: String,
        message_id: i64,
        text: String,
    },
    /// The reply is final (complete, failed, or stopped).
    Done {
        video_id: String,
        message_id: i64,
        content: String,
        error: Option<String>,
    },
}

pub type Emit = Arc<dyn Fn(ChatEvent) + Send + Sync>;

struct Run {
    abort: AbortHandle,
    reply_id: i64,
    text: Arc<Mutex<String>>,
}

/// Replies in flight, one per video at most.
#[derive(Default)]
pub struct Chats {
    runs: Mutex<HashMap<String, Run>>,
}

/// Store the question and a pending reply, then stream the answer in the background.
/// Must be called inside a Tokio runtime. Returns the thread including both new messages.
pub fn send(
    engine: &Arc<Engine>,
    video_id: &str,
    question: &str,
    emit: Emit,
) -> Result<Vec<Message>> {
    let question = question.trim();
    if question.is_empty() {
        bail!("Type a question first.");
    }
    if engine.chats.runs.lock().unwrap().contains_key(video_id) {
        bail!("Still answering the previous question. Stop it or wait.");
    }
    let (video, settings, history, reply_id, thread) = {
        let conn = engine.db.lock().unwrap();
        let video = db::video(&conn, video_id)?.context("video not found")?;
        let settings = db::get_settings(&conn)?;
        let history: Vec<(String, String)> = db::messages(&conn, video_id)?
            .into_iter()
            .filter(|m| m.error.is_none() && !m.pending)
            .map(|m| (m.role, m.content))
            .collect();
        db::add_message(&conn, video_id, "user", question, false)?;
        let reply_id = db::add_message(&conn, video_id, "assistant", "", true)?;
        (
            video,
            settings,
            history,
            reply_id,
            db::messages(&conn, video_id)?,
        )
    };
    let text = Arc::new(Mutex::new(String::new()));
    let (me, vid, q, partial) = (
        Arc::clone(engine),
        video_id.to_string(),
        question.to_string(),
        Arc::clone(&text),
    );
    let task = tokio::spawn(async move {
        let result: Result<String> = async {
            let silent: ytdlp::Progress = Box::new(|_| {});
            let (t, _) =
                ytdlp::load(&video.url, None, false, &settings.model, false, &silent).await?;
            let prompt = agent::chat_prompt(
                &settings.profile,
                &video.title,
                &video.channel,
                video.analysis.as_deref().unwrap_or("(not analyzed yet)"),
                &history,
                &q,
            );
            agent::ask_stream(&prompt, &captions::render(&t, true), |piece| {
                partial.lock().unwrap().push_str(piece);
                emit(ChatEvent::Delta {
                    video_id: vid.clone(),
                    message_id: reply_id,
                    text: piece.to_string(),
                });
            })
            .await
        }
        .await;
        me.chats.runs.lock().unwrap().remove(&vid);
        let (content, error) = match result {
            Ok(answer) => (answer, None),
            Err(e) => (partial.lock().unwrap().clone(), Some(format!("{e:#}"))),
        };
        if let Err(e) =
            db::finish_message(&me.db.lock().unwrap(), reply_id, &content, error.as_deref())
        {
            log::error!("saving chat reply: {e:#}");
        }
        emit(ChatEvent::Done {
            video_id: vid,
            message_id: reply_id,
            content,
            error,
        });
    });
    engine.chats.runs.lock().unwrap().insert(
        video_id.to_string(),
        Run {
            abort: task.abort_handle(),
            reply_id,
            text,
        },
    );
    Ok(thread)
}

/// Stop the reply in flight, keeping what was written so far.
pub fn stop(engine: &Engine, video_id: &str, emit: &Emit) -> Result<()> {
    let Some(run) = engine.chats.runs.lock().unwrap().remove(video_id) else {
        return Ok(());
    };
    run.abort.abort(); // drops the agent process (kill_on_drop)
    let content = run.text.lock().unwrap().clone();
    db::finish_message(
        &engine.db.lock().unwrap(),
        run.reply_id,
        &content,
        Some("Stopped"),
    )?;
    emit(ChatEvent::Done {
        video_id: video_id.to_string(),
        message_id: run.reply_id,
        content,
        error: Some("Stopped".into()),
    });
    Ok(())
}

pub fn clear(engine: &Engine, video_id: &str, emit: &Emit) -> Result<()> {
    stop(engine, video_id, emit)?;
    db::clear_messages(&engine.db.lock().unwrap(), video_id)
}
