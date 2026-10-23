import { useCallback, useLayoutEffect, useRef, useState } from "react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { ExpandButton } from "@/components/bits"
import { cn } from "@/lib/utils"
import { callTool, sendMessage, useHost } from "@/host"
import { isOnboarding, type OnboardingData, type Step } from "@/types"
import { followedSummary, type ActOpts, type Call, type SetupCtx } from "./ctx"
import { StepBox } from "./parts"
import { AboutStep } from "./about"
import { FollowStep } from "./follow"
import { DailyStep } from "./daily"
import { DoneStep } from "./done"

/** One box per question, in this order. */
const STEPS: ["about" | "follow" | "daily", string][] = [
  ["about", "What do you want to learn?"],
  ["follow", "What should tx follow?"],
  ["daily", "A brief every morning?"],
]

/** Setup: three questions in their own boxes, then done. Opens at the first unanswered one. */
export function Setup({ initial, onBrief }: { initial: OnboardingData; onBrief: (data: unknown) => void }) {
  const host = useHost()
  const [data, setData] = useState(initial)
  const [step, setStep] = useState<Step>(initial.step)
  const [error, setError] = useState<string | null>(null)
  const [say, setSay] = useState<string | null>(null)
  const panel = useRef<HTMLDivElement>(null)

  const refresh = useCallback(async (args: Record<string, unknown> = {}, advance = true) => {
    const r = await callTool("onboarding", args)
    const d = r.structuredContent
    if (isOnboarding(d)) {
      setData(d)
      if (advance) setStep(d.step)
    }
  }, [])

  const act = useCallback(
    async (calls: Call[], message: string, opts: ActOpts = {}) => {
      setError(null)
      setSay(null)
      if (host.canCallTools && calls.length) {
        try {
          for (const [name, args] of calls) await callTool(name, args)
          await refresh(opts.refresh, opts.advance ?? true)
          return true
        } catch (e) {
          setError(e instanceof Error ? e.message : (e as { message?: string })?.message || "That didn't work.")
          /* fall through to asking the agent */
        }
      }
      if (host.canMessage) {
        try {
          await sendMessage(message)
          setError(null)
          return false
        } catch {
          /* show it instead */
        }
      }
      setSay(message)
      return false
    },
    [host.canCallTools, host.canMessage, refresh]
  )

  /**
   * Apps give the panel a fixed height, and their fonts make the same layout taller or shorter.
   * When a step doesn't fit, drop to the compact layout rather than make the whole panel scroll.
   * `data-compact` is set here, not by React, so it can be taken off and measured again on every render.
   */
  useLayoutEffect(() => {
    const p = panel.current
    if (!p) return
    const fit = () => {
      p.removeAttribute("data-compact")
      if (p.scrollHeight > p.clientHeight + 1) p.setAttribute("data-compact", "")
    }
    fit()
    const ro = new ResizeObserver(fit)
    ro.observe(p)
    return () => ro.disconnect()
  })

  const ctx: SetupCtx = { data, step, setStep, canTools: host.canCallTools, act, refresh, onBrief }
  const done = { about: !!data.profile.trim(), follow: data.channels.some((c) => c.watching), daily: data.daily_brief }
  const summary = { about: data.profile.trim(), follow: followedSummary(data), daily: data.daily_brief ? "On" : "" }
  const form = { about: <AboutStep ctx={ctx} />, follow: <FollowStep ctx={ctx} />, daily: <DailyStep ctx={ctx} /> }

  return (
    <div
      ref={panel}
      className={cn(
        "group/setup thin flex min-h-0 flex-col gap-2 overflow-y-auto *:shrink-0 group-data-compact/setup:gap-1.5",
        step === "follow" && "min-h-[min(calc(var(--app-h)-4px),480px)]"
      )}
    >
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 px-1 pb-0.5">
        <b className="text-[15px] font-bold">Set up tx</b>
        <div className="flex items-center gap-3">
          <span className="text-[12.5px] text-faint group-data-compact/setup:hidden">Three questions, then your first brief</span>
          <ExpandButton />
        </div>
      </div>

      {STEPS.map(([id, question], i) => (
        <StepBox key={id} n={i + 1} title={question} open={step === id} done={done[id]} summary={summary[id]} onOpen={() => setStep(id)} grow={id === "follow"}>
          {form[id]}
        </StepBox>
      ))}
      {step === "done" && (
        <StepBox title="You're set" open done>
          <DoneStep ctx={ctx} />
        </StepBox>
      )}

      {error && (
        <Alert variant="destructive" className="mt-3">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      {say && (
        <Alert className="mt-3">
          <AlertTitle>Send this to your AI:</AlertTitle>
          <AlertDescription>
            <code className="font-mono text-[12.5px] leading-normal whitespace-pre-wrap text-foreground select-all">{say}</code>
          </AlertDescription>
        </Alert>
      )}
    </div>
  )
}
