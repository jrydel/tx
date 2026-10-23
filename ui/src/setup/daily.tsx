import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { followedSummary, type SetupCtx } from "./ctx"
import { H2, Hint } from "./parts"

export function DailyStep({ ctx }: { ctx: SetupCtx }) {
  const { data } = ctx
  const [time, setTime] = useState("08:00")
  const [saving, setSaving] = useState(false)
  const at = time || "08:00"
  const what = followedSummary(data) || "what you follow"

  const turnOn = async () => {
    setSaving(true)
    await ctx.act([["set_daily_brief", { at }]], `Use tx: turn on my daily brief at ${at}. Then continue my onboarding.`)
    setSaving(false)
  }

  return (
    <>
      <H2>{data.daily_brief ? "Your daily brief is on" : "Get a brief every morning?"}</H2>
      <Hint lead>It runs on this computer, even when this app is closed.</Hint>
      {!data.can_schedule && (
        <Hint>This needs Claude Code or Codex installed. You can still ask for your brief here any time.</Hint>
      )}
      <div className="mt-3 flex flex-wrap items-center gap-2.5 group-data-compact/setup:mt-2.5">
        <Input type="time" value={time} onChange={(e) => setTime(e.target.value)} aria-label="Time" className="w-auto" />
        <Button onClick={turnOn} disabled={!data.can_schedule || saving}>
          {saving && <Spinner data-icon="inline-start" />}
          Turn on daily brief
        </Button>
        <Button variant="ghost" className="font-semibold text-muted-foreground" onClick={() => ctx.setStep("done")}>
          Not now
        </Button>
      </div>
      {!data.daily_brief && (
        <Hint className="mt-3">
          Tomorrow at {at}, tx checks {what}, your AI screens what's new, and a notification tells you what's worth your time.
        </Hint>
      )}
    </>
  )
}
