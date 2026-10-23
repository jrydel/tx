import { useState } from "react"
import { Button } from "@/components/ui/button"
import { CardContent, CardFooter } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import type { SetupCtx } from "./ctx"

const EXAMPLES: [string, string][] = [
  ["Engineer", "I'm a backend engineer building LLM features. I want concrete agent and eval techniques and notable model launches; skip hype and drama."],
  ["Designer", "Product designer at a startup. I follow AI tools that change how products get built, plus good interface craft. No coding deep dives."],
  ["Founder", "Founder in fintech. Market moves, regulation and how others ship AI products; skip tutorials."],
]

export function AboutStep({ ctx }: { ctx: SetupCtx }) {
  const [text, setText] = useState(ctx.data.profile)
  const [saving, setSaving] = useState(false)

  const save = async () => {
    const profile = text.trim()
    if (!profile) return
    setSaving(true)
    await ctx.act([["set_profile", { profile }]], `Use tx: save my profile: "${profile}". Then continue my onboarding.`)
    setSaving(false)
  }

  return (
    <>
      <CardContent className="grid gap-4 group-data-compact/setup:gap-2.5">
        <Textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          aria-label="About you"
          placeholder="Who you are, what you want to learn, what to skip"
          className="field-sizing-fixed min-h-[104px] resize-y px-3.5 py-3 text-[15px] leading-relaxed group-data-compact/setup:min-h-[60px] group-data-compact/setup:text-sm"
        />
        <div className="grid gap-2">
          <p className="m-0 text-[13px] font-medium text-muted-foreground">Or start from an example</p>
          <div className="grid grid-cols-3 gap-2.5 max-[600px]:grid-cols-1 group-data-compact/setup:flex group-data-compact/setup:flex-wrap group-data-compact/setup:gap-1.5">
            {EXAMPLES.map(([role, x]) => (
              <Button
                key={role}
                type="button"
                variant="outline"
                aria-pressed={text === x}
                title={`${x}\n\nClick to start from this, then make it yours.`}
                onClick={() => setText(x)}
                className="h-auto flex-col items-start justify-start gap-1 rounded-lg px-3.5 py-3 text-left text-[13px] leading-[1.45] font-normal whitespace-normal text-muted-foreground hover:text-foreground aria-pressed:border-foreground/40 aria-pressed:bg-muted aria-pressed:text-foreground group-data-compact/setup:h-7 group-data-compact/setup:rounded-full group-data-compact/setup:px-3 group-data-compact/setup:py-0"
              >
                <b className="text-[11px] font-semibold tracking-[.07em] text-faint uppercase group-data-compact/setup:text-[12px] group-data-compact/setup:tracking-normal group-data-compact/setup:text-muted-foreground group-data-compact/setup:normal-case">
                  {role}
                </b>
                <span className="line-clamp-3 group-data-compact/setup:hidden">{x}</span>
              </Button>
            ))}
          </div>
        </div>
      </CardContent>
      <CardFooter className="justify-between gap-3">
        <span className="text-[13px] text-faint group-data-compact/setup:hidden">You can change it any time.</span>
        <Button onClick={save} disabled={saving || !text.trim()} className="ml-auto">
          {saving && <Spinner data-icon="inline-start" />}
          Save and continue
        </Button>
      </CardFooter>
    </>
  )
}
