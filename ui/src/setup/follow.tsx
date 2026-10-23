import { useState } from "react"
import { SparklesIcon, XIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { callTool } from "@/host"
import type { Pick } from "@/types"
import type { SetupCtx } from "./ctx"
import { Hint } from "./parts"
import { SourceSearch } from "./search"

const BROWSERS = ["Chrome", "Firefox", "Safari", "Edge", "Brave"]

/**
 * Search is the way in. What's followed shows as chips; subscriptions imported from a browser (and the AI's picks
 * among them) show as a checklist to tick.
 */
export function FollowStep({ ctx }: { ctx: SetupCtx }) {
  const { data, act } = ctx
  const watching = data.channels.filter((c) => c.watching)
  const candidates = data.channels.filter((c) => !c.watching)
  const [showImport, setShowImport] = useState(data.channels.length === 0)
  const [browser, setBrowser] = useState("chrome")
  const [importing, setImporting] = useState(false)
  const [suggesting, setSuggesting] = useState(false)
  const [note, setNote] = useState<string | null>(null)
  const [filter, setFilter] = useState("")
  const [overrides, setOverrides] = useState<ReadonlyMap<string, boolean>>(new Map())
  const [saving, setSaving] = useState(false)

  const isPicked = (c: { id: string; suggested?: boolean }) => overrides.get(c.id) ?? !!c.suggested
  const picked = candidates.filter(isPicked)
  const picks = candidates.filter((c) => c.suggested).length
  const q = filter.trim().toLowerCase()

  const follow = (ref: string, name: string) =>
    act([["watch_channel", { channels: [ref] }]], `Use tx: follow ${name}. Then continue my onboarding.`, { advance: false })

  const unfollow = (id: string, name: string) =>
    act([["unwatch_channel", { channel_id: id }]], `Use tx: stop following ${name} (${id}). Then continue my onboarding.`, { advance: false })

  const runImport = async () => {
    setImporting(true)
    setNote(null)
    const ok = await act(
      [["import_subscriptions", { browser }]],
      `Use tx: import my YouTube subscriptions from ${browser}. Then continue my onboarding with your suggestions for my profile.`,
      { advance: false, refresh: { step: "follow" } }
    )
    setImporting(false)
    if (ok) {
      setShowImport(false)
      setNote("Imported. Tick what to follow, or let your AI pick.")
    }
  }

  const suggest = async () => {
    setSuggesting(true)
    setNote(null)
    if (!ctx.canTools) {
      await act([], "Use tx: pick which of my imported channels and shows fit my profile, and show onboarding with your picks.")
      setSuggesting(false)
      return
    }
    try {
      const r = await callTool("suggest_channels", {})
      const found = (r.structuredContent as { picks?: Pick[] } | undefined)?.picks ?? []
      await ctx.refresh({ step: "follow", suggest: found }, false)
      if (!found.length) setNote("Your AI didn't find a clear fit. Tick what you like.")
    } catch (e) {
      await act([], "Use tx: pick which of my imported channels and shows fit my profile, and show onboarding with your picks.")
      setNote(e instanceof Error ? e.message : "Your AI couldn't pick.")
    }
    setSuggesting(false)
  }

  const followTicked = async () => {
    setSaving(true)
    const ids = picked.map((c) => c.id)
    await act([["watch_channel", { channels: ids }]], `Use tx: follow these (ids): ${ids.join(", ")}. Then continue my onboarding.`)
    setSaving(false)
  }

  const importRow = (
    <div className="mt-2.5 flex flex-wrap items-center gap-2">
      <span className="text-[13px] text-muted-foreground">Or import your YouTube subscriptions from</span>
      <select
        aria-label="Browser signed in to YouTube"
        value={browser}
        onChange={(e) => setBrowser(e.target.value)}
        className="h-8 rounded-lg border border-input bg-card px-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        {BROWSERS.map((b) => (
          <option key={b} value={b.toLowerCase()}>
            {b}
          </option>
        ))}
      </select>
      <Button variant="outline" onClick={runImport} disabled={importing}>
        {importing && <Spinner data-icon="inline-start" />}
        {importing ? "Importing…" : "Import"}
      </Button>
    </div>
  )

  const actions = (
    <div className="mt-2.5 flex flex-wrap items-center gap-2.5">
      {candidates.length > 0 && (
        <Button onClick={followTicked} disabled={!picked.length || saving}>
          {saving && <Spinner data-icon="inline-start" />}
          Follow {picked.length}
        </Button>
      )}
      <Button variant={candidates.length ? "outline" : "default"} disabled={!watching.length} onClick={() => ctx.setStep("daily")}>
        Continue
      </Button>
      {!watching.length && <span className="text-[13px] text-faint">Follow at least one to continue.</span>}
      {data.channels.length > 0 && !showImport && (
        <Button variant="ghost" className="font-semibold text-muted-foreground" onClick={() => setShowImport(true)}>
          Import subscriptions
        </Button>
      )}
    </div>
  )

  return (
    <>
      <Hint lead>Only what you follow gets analyzed. Search for a channel or podcast; you can add more later.</Hint>
      <SourceSearch channels={data.channels} canSearch={ctx.canTools} onFollow={follow} />

      {watching.length > 0 && (
        <div className="mt-2.5 flex items-start gap-2">
          <span className="shrink-0 pt-[3px] text-[11px] font-bold tracking-[.07em] whitespace-nowrap text-faint uppercase">Following {watching.length}</span>
          <ul className="thin m-0 flex max-h-[54px] min-w-0 flex-1 list-none flex-wrap gap-1.5 overflow-y-auto p-0 group-data-compact/setup:max-h-[28px]">
            {watching.map((c) => (
              <li key={c.id}>
                <Badge variant="secondary" className="h-6 gap-1 pr-0.5 pl-2.5">
                  <span className="max-w-40 truncate">{c.title}</span>
                  <button
                    type="button"
                    aria-label={`Stop following ${c.title}`}
                    title={`Stop following ${c.title}`}
                    onClick={() => unfollow(c.id, c.title)}
                    className="grid size-4 cursor-pointer place-items-center rounded-full text-muted-foreground hover:bg-foreground hover:text-card"
                  >
                    <XIcon className="size-3" />
                  </button>
                </Badge>
              </li>
            ))}
          </ul>
        </div>
      )}

      {showImport ? importRow : null}
      {note && <p className="m-0 mt-2 text-[13px] text-muted-foreground">{note}</p>}

      {candidates.length > 0 && (
        <div className="mt-2.5 flex min-h-[172px] flex-1 shrink-0 basis-0 flex-col group-data-compact/setup:min-h-[112px]">
          <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
            {suggesting ? (
              <Hint className="m-0 flex items-center gap-2">
                <Spinner /> Your AI is reading your list. This takes about 30 seconds.
              </Hint>
            ) : picks ? (
              <Hint className="m-0">Your AI picked {picks} that fit you. Change anything, then follow.</Hint>
            ) : (
              <Hint className="m-0">Tick what to follow from your {candidates.length}, or let your AI pick.</Hint>
            )}
            <div className="flex items-center gap-2">
              {!picks && !suggesting && (
                <Button variant="outline" size="sm" onClick={suggest}>
                  <SparklesIcon data-icon="inline-start" />
                  Let your AI pick
                </Button>
              )}
              <Input
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder={`Filter ${candidates.length}`}
                aria-label="Filter channels and shows"
                className="h-7 w-40 px-2.5 text-[13px]"
              />
            </div>
          </div>
          <div className="thin mt-2 min-h-24 flex-1 overflow-y-auto rounded-lg border">
            {candidates.map((c) => (
              <label
                key={c.id}
                hidden={!!q && !c.title.toLowerCase().includes(q)}
                className="grid cursor-pointer grid-cols-[18px_1fr_auto] items-start gap-2.5 border-t px-[11px] py-2 first:border-t-0 hover:bg-muted"
              >
                <Checkbox
                  checked={isPicked(c)}
                  aria-label={c.title}
                  className="mt-[3px]"
                  onCheckedChange={(v) => setOverrides((m) => new Map(m).set(c.id, v === true))}
                />
                <span className="min-w-0">
                  <span className="block truncate text-[13.5px] font-semibold">
                    {c.suggested && <span className="mr-1.5 text-[11px] font-bold tracking-[.05em] text-watch uppercase">Pick</span>}
                    {c.title}
                  </span>
                  {c.why && <span className="block text-[12.5px] text-muted-foreground">{c.why}</span>}
                </span>
                <span className="text-[11px] font-semibold tracking-[.05em] text-faint uppercase">{c.podcast ? "Podcast" : "YouTube"}</span>
              </label>
            ))}
          </div>
          {actions}
        </div>
      )}

      {candidates.length === 0 && actions}
    </>
  )
}
