import { useState } from "react"
import { Button } from "@/components/ui/button"
import { CardContent, CardFooter } from "@/components/ui/card"
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { followedSummary, type SetupCtx } from "./ctx"
import { Hint } from "./parts"

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
      <CardContent className="grid gap-3">
        {!data.can_schedule && <Hint className="m-0">This needs Claude Code or Codex installed. You can still ask for your brief here any time.</Hint>}
        <Field>
          <FieldLabel htmlFor="brief-time">Every morning at</FieldLabel>
          <Input id="brief-time" type="time" value={time} onChange={(e) => setTime(e.target.value)} className="h-10 w-36! text-[15px]" />
          {!data.daily_brief && (
            <FieldDescription>
              tx checks {what}, your AI screens what's new, and a notification tells you what's worth your time.
            </FieldDescription>
          )}
        </Field>
      </CardContent>
      <CardFooter className="justify-end gap-2.5">
        <Button variant="ghost" className="text-muted-foreground" onClick={() => ctx.setStep("done")}>
          Not now
        </Button>
        <Button onClick={turnOn} disabled={!data.can_schedule || saving}>
          {saving && <Spinner data-icon="inline-start" />}
          Turn on daily brief
        </Button>
      </CardFooter>
    </>
  )
}
