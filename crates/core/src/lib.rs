//! tx engine: transcripts (captions, published podcast transcripts or local Whisper), channel and podcast watching, analyses written by the
//! user's own agent, and digests. Used by the `tx` CLI and its MCP server.

pub mod agent;
pub mod captions;
pub mod db;
pub mod debrief;
pub mod details;
pub mod feeds;
pub mod paths;
pub mod podcasts;
pub mod report;
pub mod whisper;
pub mod ytdlp;
