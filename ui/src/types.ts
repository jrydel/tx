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

export interface BriefData {
  videos: Card[]
  profile?: string
}

export interface Channel {
  id: string
  title: string
  podcast: boolean
  watching: boolean
  suggested?: boolean
  why?: string
}

export type Step = "about" | "follow" | "daily" | "done"

export interface OnboardingData {
  view: "onboarding"
  step: Step
  profile: string
  channels: Channel[]
  daily_brief: boolean
  can_schedule: boolean
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
  !!d && typeof d === "object" && (d as { view?: unknown }).view === "onboarding"

export const isBrief = (d: unknown): d is BriefData =>
  !!d && typeof d === "object" && Array.isArray((d as { videos?: unknown }).videos)
