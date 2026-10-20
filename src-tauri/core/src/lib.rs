//! tx engine: transcripts (captions or local Whisper), channel watching, and analysis.
//! Shared by the desktop app and the `tx` CLI; knows nothing about Tauri.

pub mod agent;
pub mod captions;
pub mod db;
pub mod engine;
pub mod feeds;
pub mod paths;
pub mod whisper;
pub mod ytdlp;
