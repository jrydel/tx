//! tx engine: transcripts (captions, published podcast transcripts or local Whisper), channel and podcast watching, analyses written by the
//! user's own agent, digests, and a profile learned from the user's coding-agent sessions. Used by the `tx` CLI and its MCP server.

pub mod agent;
pub mod captions;
pub mod db;
pub mod debrief;
pub mod details;
pub mod feeds;
pub mod learn;
pub mod paths;
pub mod podcasts;
pub mod report;
pub mod sessions;
pub mod tts;
pub mod whisper;
pub mod ytdlp;
