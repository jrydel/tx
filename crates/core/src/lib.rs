//! tx engine: transcripts (captions or local Whisper), channel watching, analyses written by the
//! user's own agent, and digests. Used by the `tx` CLI and its MCP server.

pub mod agent;
pub mod captions;
pub mod db;
pub mod feeds;
pub mod paths;
pub mod report;
pub mod whisper;
pub mod ytdlp;
