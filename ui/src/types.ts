/** Shapes of the `structuredContent` that tx's tools return. */

export interface Takeaway {
  text: string
  at: string
  url?: string
}

export type Advice = "watch" | "skim" | "skip"

export interface Card {
  id: string
  title: string
  channel: string
  url: string
  published: string
  podcast: boolean
  length?: string
  thumbnail?: string | null
  /** new: waiting to be analyzed; failed: couldn't be (no transcript…); passed: judged not worth transcribing. */
  state: "new" | "done" | "failed" | "passed"
  /** Why, when failed or passed. */
  error?: string | null
  analyzed: boolean
  relevance?: number
  verdict?: string
  tldr?: string
  takeaways?: Takeaway[]
  advice?: Advice
  skim_from?: string
  skim_to?: string
  skim_url?: string
  reason?: string
}

/** What the week's analyzed videos add up to, written by the user's own AI after a screening. */
export interface Debrief {
  headline: string
  summary: string
  threads: { title: string; text: string; video_ids: string[] }[]
  /** The analyzed videos it was written over. */
  video_ids: string[]
  written_at: string
  /** The narrated show ("Watch your debrief"), when the AI wrote one. */
  script?: Script | null
}

/**
 * The narrated debrief video, written by the user's AI: one chapter (segment) per channel, each a takeaway and then
 * the key parts, every part set up by a narrator line and then played as it is.
 */
export interface Script {
  intro: string
  segments: {
    video_id: string
    intro: string
    parts: {
      say: string
      moment_from: string
      moment_to: string
      moment_start: number
      moment_end: number
    }[]
  }[]
  outro?: string | null
}

export interface BriefData {
  videos: Card[]
  profile?: string
  debrief?: Debrief | null
  /** Set when this is the brief of the last `days` days, rather than a list of picked videos. */
  days?: number
}

export interface Channel {
  id: string
  title: string
  podcast: boolean
  watching: boolean
  suggested?: boolean
  why?: string
  /** Avatar or show art, and "@handle · 4.3M subscribers": null until tx has fetched them (channel_details). */
  thumbnail?: string | null
  subtitle?: string | null
}

export type Step = "about" | "follow" | "daily" | "done"

export interface OnboardingData {
  view: "onboarding"
  step: Step
  profile: string
  channels: Channel[]
  daily_brief: boolean
  can_schedule: boolean
  /** Claude Code, Codex or Oh My Pi keep sessions on this computer, so tx can learn from them. */
  can_learn: boolean
  /** What tx learned from those sessions, if the user turned it on. */
  learned: { at: string; days: number } | null
}

export interface SourceResult {
  kind: "youtube" | "podcast"
  ref: string
  title: string
  subtitle: string
  thumbnail: string | null
  watching: boolean
}

export interface Pick {
  id: string
  why: string
}

export interface ToolResult {
  content?: { type: string; text?: string }[]
  structuredContent?: unknown
  isError?: boolean
}

export const isOnboarding = (d: unknown): d is OnboardingData =>
  !!d &&
  typeof d === "object" &&
  (d as { view?: unknown }).view === "onboarding"

export const isBrief = (d: unknown): d is BriefData =>
  !!d &&
  typeof d === "object" &&
  Array.isArray((d as { videos?: unknown }).videos)
