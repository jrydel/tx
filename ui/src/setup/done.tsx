import { useEffect, useState } from "react"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import { Spinner } from "@/components/ui/spinner"
import { callTool, textOf } from "@/host"
import { isBrief } from "@/types"
import type { SetupCtx } from "./ctx"
import { Hint } from "./parts"

type Phase =
  | { kind: "idle" }
  | { kind: "starting" }
  | { kind: "screening"; total: number; left: number }
  | { kind: "loading" }
  | { kind: "empty"; message: string }

const ASK_MESSAGE =
  "Use tx: check what I follow for new uploads, screen them and show me my brief. If nothing is new yet, give me a TL;DR of the latest uploads instead."

/** You're set. "Show my first brief" has the user's AI screen what's new, waits for it, and shows the brief in place. */
export function DoneStep({ ctx }: { ctx: SetupCtx }) {
  const [phase, setPhase] = useState<Phase>({ kind: "idle" })

  const showBrief = async () => {
    setPhase({ kind: "loading" })
    try {
      const r = await callTool("show_videos", {})
      if (isBrief(r.structuredContent) && r.structuredContent.videos.length) return ctx.onBrief(r.structuredContent)
      setPhase({ kind: "empty", message: "" })
    } catch (e) {
      setPhase({ kind: "empty", message: e instanceof Error ? e.message : "" })
    }
  }

  const start = async () => {
    if (!ctx.canTools) {
      await ctx.act([], ASK_MESSAGE)
      return
    }
    setPhase({ kind: "starting" })
    try {
      const r = await callTool("screen_now", {})
      const s = r.structuredContent as { started?: boolean; waiting?: number; reason?: string } | undefined
      if (s?.started) setPhase({ kind: "screening", total: Math.max(1, s.waiting ?? 1), left: s.waiting ?? 1 })
      else await showBrief()
    } catch (e) {
      setPhase({ kind: "empty", message: e instanceof Error ? e.message : "" })
    }
  }

  // While your AI works: ask tx how much is still waiting every 5 s, and show the brief when nothing is.
  const screening = phase.kind === "screening"
  useEffect(() => {
    if (!screening) return
    let live = true
    let timer: ReturnType<typeof setTimeout>
    const poll = async () => {
      try {
        const left = Number(JSON.parse(textOf(await callTool("status", {}))).waiting_for_analysis)
        if (!live) return
        if (left === 0) return void showBrief()
        if (!isNaN(left)) setPhase((p) => (p.kind === "screening" ? { ...p, left } : p))
      } catch {
        /* keep waiting; the next poll may get through */
      }
      if (live) timer = setTimeout(poll, 5000)
    }
    timer = setTimeout(poll, 5000)
    return () => {
      live = false
      clearTimeout(timer)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [screening])

  const busy = phase.kind === "starting" || phase.kind === "loading" || phase.kind === "screening"

  return (
    <>
      {phase.kind === "screening" ? (
        <div className="grid gap-2" role="status">
          <Progress value={((phase.total - Math.min(phase.left, phase.total)) / phase.total) * 100} aria-label="Screening progress" />
          <p className="m-0 flex items-center gap-2 text-[13.5px] text-muted-foreground">
            <Spinner />
            Your AI is screening {phase.left === 1 ? "1 new video" : `${phase.left} new videos`}. This takes a few minutes; it keeps going if you close this.
          </p>
        </div>
      ) : phase.kind === "empty" ? (
        <Hint className="mb-0">
          Nothing to show yet{phase.message ? `: ${phase.message.replace(/[.\s]+$/, "")}` : ""}. Paste a video link in the chat and I'll give you a TL;DR.
        </Hint>
      ) : (
        <Hint lead className="mb-0">
          Ask for your brief any time, or paste any video link for a TL;DR.
        </Hint>
      )}

      {phase.kind !== "screening" && (
        <div className="mt-3 flex flex-wrap items-center gap-2.5 group-data-compact/setup:mt-2.5">
          <Button onClick={start} disabled={busy}>
            {busy && <Spinner data-icon="inline-start" />}
            {phase.kind === "empty" ? "Try again" : "Show my first brief"}
          </Button>
        </div>
      )}
    </>
  )
}
