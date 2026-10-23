import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import type { SetupCtx } from "./ctx"
import { H2, Hint } from "./parts"

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
      <H2>Who are you, and what do you want to learn?</H2>
      <Hint lead>Every video and episode is scored against this. Name your role, the topics you care about and what to skip.</Hint>
      <Textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        aria-label="About you"
        placeholder="Who you are, what you want to learn, what to skip"
        className="field-sizing-fixed min-h-[84px] resize-y leading-normal group-data-compact/setup:min-h-[60px]"
      />
      <div className="mt-2.5 grid grid-cols-3 gap-2 max-[600px]:grid-cols-1 group-data-compact/setup:flex group-data-compact/setup:flex-wrap group-data-compact/setup:items-center group-data-compact/setup:gap-1.5">
        <p className="col-span-3 m-0 text-[13.5px] text-muted-foreground group-data-compact/setup:mr-1">Or start from an example:</p>
        {EXAMPLES.map(([role, x]) => (
          <Button
            key={role}
            type="button"
            variant="outline"
            title={`${x}\n\nClick to start from this, then make it yours.`}
            onClick={() => setText(x)}
            className="h-auto flex-col items-start justify-start gap-0.5 rounded-lg px-2.5 py-2 text-left text-[12.5px] leading-[1.4] font-normal whitespace-normal text-muted-foreground hover:border-faint group-data-compact/setup:h-6 group-data-compact/setup:rounded-full group-data-compact/setup:px-2.5 group-data-compact/setup:py-0"
          >
            <b className="text-[11px] font-bold tracking-[.06em] text-faint uppercase group-data-compact/setup:text-[11.5px] group-data-compact/setup:tracking-normal group-data-compact/setup:text-muted-foreground group-data-compact/setup:normal-case">
              {role}
            </b>
            <span className="line-clamp-3 group-data-compact/setup:hidden">{x}</span>
          </Button>
        ))}
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-2.5 group-data-compact/setup:mt-2.5">
        <Button onClick={save} disabled={saving || !text.trim()}>
          {saving && <Spinner data-icon="inline-start" />}
          Save and continue
        </Button>
      </div>
    </>
  )
}
