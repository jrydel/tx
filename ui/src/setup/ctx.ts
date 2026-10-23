import type { OnboardingData, Step } from "@/types"

export type Call = [name: string, args: Record<string, unknown>]

export interface ActOpts {
  /** After the calls, jump to the step the server says is next (default). False keeps the user where they are. */
  advance?: boolean
  /** Arguments for the `onboarding` refresh that follows the calls. */
  refresh?: Record<string, unknown>
}

export interface SetupCtx {
  data: OnboardingData
  step: Step
  setStep: (s: Step) => void
  canTools: boolean
  /**
   * Carry out a choice: call the tools and refresh the panel when the app allows it, else ask the agent in
   * the chat, else show what to say. Resolves true when the tools ran.
   */
  act: (calls: Call[], message: string, opts?: ActOpts) => Promise<boolean>
  /** Re-read the panel's data from tx (`onboarding`), optionally following the server's next step. */
  refresh: (args?: Record<string, unknown>, advance?: boolean) => Promise<void>
  onBrief: (data: unknown) => void
}

/** "2 channels", "1 show", or "3 channels and 1 show". */
export function followedSummary(data: OnboardingData): string {
  const watching = data.channels.filter((c) => c.watching)
  const shows = watching.filter((c) => c.podcast).length
  const count = (k: number, w: string) => (k ? `${k} ${w}${k > 1 ? "s" : ""}` : "")
  return [count(watching.length - shows, "channel"), count(shows, "show")].filter(Boolean).join(" and ")
}
