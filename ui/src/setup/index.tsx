import { useCallback, useLayoutEffect, useRef, useState } from "react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { ExpandButton } from "@/components/bits"
import { cn } from "@/lib/utils"
import { callTool, sendMessage, useHost } from "@/host"
import { isOnboarding, type OnboardingData, type Step } from "@/types"
import type { ActOpts, Call, SetupCtx } from "./ctx"
import { AboutStep } from "./about"
import { FollowStep } from "./follow"
import { DailyStep } from "./daily"
import { DoneStep } from "./done"

const STEPS: ["about" | "follow" | "daily", string][] = [
  ["about", "About you"],
  ["follow", "What to follow"],
  ["daily", "Daily brief"],
]

/** Setup: about you, what to follow, the daily brief, then done. Starts at the first unfinished step. */
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

  return (
    <div
      ref={panel}
      className={cn(
        "group/setup thin flex min-h-0 flex-col overflow-y-auto rounded-xl border bg-card px-[18px] py-3.5 data-compact:py-3 *:shrink-0",
        step === "follow" && "min-h-[min(calc(var(--app-h)-4px),480px)]"
      )}
    >
      <div className="mb-3 flex flex-wrap items-center justify-between gap-x-4 gap-y-1 group-data-compact/setup:mb-2">
        <b className="text-[15px] font-bold">Set up tx</b>
        <div className="flex items-center gap-3">
          <span className="text-[12.5px] text-faint group-data-compact/setup:hidden">Two minutes, then your first brief</span>
          <ExpandButton />
        </div>
      </div>

      <nav aria-label="Setup steps" className="mb-4 grid grid-cols-3 group-data-compact/setup:mb-2.5">
        {STEPS.map(([id, label]) => (
          <button
            key={id}
            type="button"
            aria-current={id === step ? "step" : undefined}
            data-done={done[id] || undefined}
            title={done[id] ? `${label}: done` : label}
            onClick={() => setStep(id)}
            className={cn(
              "group/stop relative grid cursor-pointer justify-items-start gap-[7px] text-left text-[12.5px] font-semibold text-faint hover:text-foreground aria-[current=step]:text-foreground",
              "after:absolute after:top-1.5 after:right-0.5 after:left-3.5 after:h-0.5 after:rounded-[1px] after:bg-border last:after:hidden data-done:after:bg-watch"
            )}
          >
            <i
              aria-hidden
              className="relative z-10 size-3.5 rounded-full border-2 border-border bg-card group-data-done/stop:border-watch group-data-done/stop:bg-watch group-aria-[current=step]/stop:border-foreground group-aria-[current=step]/stop:shadow-[0_0_0_4px_color-mix(in_srgb,var(--foreground)_14%,transparent)] group-data-done/stop:group-aria-[current=step]/stop:border-watch"
            />
            {label}
          </button>
        ))}
      </nav>

      {step === "about" && <AboutStep ctx={ctx} />}
      {step === "follow" && <FollowStep ctx={ctx} />}
      {step === "daily" && <DailyStep ctx={ctx} />}
      {step === "done" && <DoneStep ctx={ctx} done={done} />}

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
