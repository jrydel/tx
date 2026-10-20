import { invoke } from "@tauri-apps/api/core"

// Mirrors the Rust structs in src-tauri/src/db.rs and commands.rs.

export type VideoStatus = "skipped" | "queued" | "working" | "done" | "error"

export type Video = {
  id: string
  title: string
  url: string
  channel_id: string
  channel: string
  published: string
  status: VideoStatus
  relevance: number | null
  verdict: string | null
  analysis: string | null
  error: string | null
  duration: number | null
  analyzed_at: string | null
}

export type Status = {
  now: "idle" | "checking" | "analyzing"
  video: string | null
  detail: string | null
  checked_at: string | null
  watched: number
  channels: number
  queued: number
  agent: boolean
  profile_set: boolean
}

export type Channel = {
  id: string
  title: string
  enabled: boolean
  checked_at: string | null
  analyzed: number
}

export type Settings = {
  profile: string
  browser: string
  interval_min: number
  notify_min: number
  model: string
}

export type Transcript = {
  source: string
  duration: number | null
  paragraphs: { start: number; text: string }[]
}

export type View = "analyzed" | "earlier"

export type Message = {
  id: number
  role: "user" | "assistant"
  content: string
  pending: boolean
  error: string | null
  created_at: string
}

/** Emitted on the "chat" event while a reply streams (mirrors chat::ChatEvent). */
export type ChatEvent =
  | { kind: "delta"; video_id: string; message_id: number; text: string }
  | {
      kind: "done"
      video_id: string
      message_id: number
      content: string
      error: string | null
    }

export const api = {
  status: () => invoke<Status>("status"),
  videos: (view: View) => invoke<Video[]>("videos", { view }),
  video: (id: string) => invoke<Video>("video", { id }),
  transcript: (id: string) => invoke<Transcript>("transcript", { id }),
  analyze: (id: string) => invoke<Video>("analyze", { id }),
  check: () => invoke<void>("check"),
  channels: () => invoke<Channel[]>("channels"),
  setWatching: (id: string, enabled: boolean) =>
    invoke<void>("set_watching", { id, enabled }),
  importSubscriptions: (browser: string) =>
    invoke<number>("import_subscriptions", { browser }),
  addChannel: (reference: string) =>
    invoke<string>("add_channel", { reference }),
  settings: () => invoke<Settings>("settings"),
  saveSettings: (settings: Settings) =>
    invoke<Settings>("save_settings", { settings }),
  models: () => invoke<string[]>("models"),
  autostart: () => invoke<boolean>("autostart"),
  setAutostart: (enabled: boolean) =>
    invoke<boolean>("set_autostart", { enabled }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  chat: (id: string) => invoke<Message[]>("chat", { id }),
  chatSend: (id: string, question: string) =>
    invoke<Message[]>("chat_send", { id, question }),
  chatStop: (id: string) => invoke<void>("chat_stop", { id }),
  chatClear: (id: string) => invoke<void>("chat_clear", { id }),
}
