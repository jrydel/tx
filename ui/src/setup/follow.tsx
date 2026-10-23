import { useState, type ReactNode } from "react"
import { SparklesIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { CardContent, CardFooter } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { callTool } from "@/host"
import type { Channel, Pick } from "@/types"
import type { SetupCtx } from "./ctx"
import { Hint } from "./parts"
import { SourceSearch } from "./search"
import { FollowingMark, SourceLine, useChannelDetails } from "./source-row"

const BROWSERS = ["Chrome", "Firefox", "Safari", "Edge", "Brave"]

/**
 * Search is the way in. Below it, one list: what you follow, then your imported subscriptions, every row the way search
 * shows it (avatar, handle, subscribers) with its own Follow. Your AI can pick from the subscriptions; its picks follow
 * in one go.
 */
export function FollowStep({ ctx }: { ctx: SetupCtx }) {
  const { data, act } = ctx
  const [showImport, setShowImport] = useState(data.channels.length === 0)
  const [browser, setBrowser] = useState("chrome")
  const [importing, setImporting] = useState(false)
  const [suggesting, setSuggesting] = useState(false)
  const [note, setNote] = useState<string | null>(null)
  const [filter, setFilter] = useState("")
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set())
  const asRow = useChannelDetails(data.channels, ctx.canTools)

  const q = filter.trim().toLowerCase()
  const shown = (c: Channel) => !q || c.title.toLowerCase().includes(q)
  const watching = data.channels.filter((c) => c.watching)
  // The AI's picks first, then the rest in the order tx sent them.
  const candidates = data.channels.filter((c) => !c.watching).sort((a, b) => Number(!!b.suggested) - Number(!!a.suggested))
  const picks = candidates.filter((c) => c.suggested)

  /** Runs `calls` for these ids, with their rows showing a spinner meanwhile. */
  const busy = async (ids: string[], calls: Parameters<typeof act>[0], message: string) => {
    setPending((p) => new Set([...p, ...ids]))
    const ok = await act(calls, message, { advance: false })
    setPending((p) => {
      const n = new Set(p)
      for (const id of ids) n.delete(id)
      return n
    })
    return ok
  }

  const followRef = (ref: string, name: string) =>
    act([["watch_channel", { channels: [ref] }]], `Use tx: follow ${name}. Then continue my onboarding.`, { advance: false })
  const follow = (c: Channel) => busy([c.id], [["watch_channel", { channels: [c.id] }]], `Use tx: follow ${c.title}. Then continue my onboarding.`)
  const unfollow = (c: Channel) =>
    busy([c.id], [["unwatch_channel", { channel_id: c.id }]], `Use tx: stop following ${c.title} (${c.id}). Then continue my onboarding.`)
  const followPicks = () =>
    busy(
      picks.map((c) => c.id),
      [["watch_channel", { channels: picks.map((c) => c.id) }]],
      `Use tx: follow these (ids): ${picks.map((c) => c.id).join(", ")}. Then continue my onboarding.`
    )

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
      setNote("Imported. Follow what you like, or let your AI pick.")
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
      if (!found.length) setNote("Your AI didn't find a clear fit. Follow what you like.")
    } catch (e) {
      await act([], "Use tx: pick which of my imported channels and shows fit my profile, and show onboarding with your picks.")
      setNote(e instanceof Error ? e.message : "Your AI couldn't pick.")
    }
    setSuggesting(false)
  }

  const importRow = (
    <div className="mt-3 flex flex-wrap items-center gap-2">
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

  const section = (title: string, count: number, tools?: ReactNode) => (
    <div className="sticky top-0 z-10 flex flex-wrap items-center justify-between gap-x-3 gap-y-1.5 border-b bg-card/95 px-3.5 py-2 backdrop-blur">
      <span className="text-[11px] font-semibold tracking-[.07em] text-faint uppercase">
        {title} · {count}
      </span>
      {tools}
    </div>
  )

  const row = (c: Channel, action: ReactNode) => (
    <li key={c.id} className="flex items-center gap-3 border-b px-3.5 py-2.5 last:border-b-0 group-data-compact/setup:py-2">
      <SourceLine s={asRow(c)} pick={c.suggested && !c.watching} />
      {action}
    </li>
  )

  const shownWatching = watching.filter(shown)
  const shownCandidates = candidates.filter(shown)
  const hasList = data.channels.length > 0

  const pickTools = suggesting ? (
    <span className="flex items-center gap-2 text-[12.5px] text-muted-foreground">
      <Spinner /> Your AI is reading your list. About 30 seconds.
    </span>
  ) : picks.length ? (
    <Button size="sm" onClick={followPicks} disabled={picks.some((c) => pending.has(c.id))}>
      {picks.some((c) => pending.has(c.id)) && <Spinner data-icon="inline-start" />}
      Follow your AI's {picks.length} {picks.length === 1 ? "pick" : "picks"}
    </Button>
  ) : (
    <Button variant="outline" size="sm" onClick={suggest}>
      <SparklesIcon data-icon="inline-start" />
      Let your AI pick
    </Button>
  )

  return (
    <>
      <CardContent className="flex min-h-0 flex-1 flex-col *:shrink-0">
        {/* Its command root is h-full; in a card of definite height that would take all of it. */}
        <div>
          <SourceSearch channels={data.channels} canSearch={ctx.canTools} onFollow={followRef} />
        </div>
        {showImport ? importRow : null}
        {note && <p className="m-0 mt-2.5 text-[13px] text-muted-foreground">{note}</p>}

        {hasList && (
          <div className="mt-4 flex min-h-[200px] flex-1 shrink-0 basis-0 flex-col group-data-compact/setup:mt-2.5 group-data-compact/setup:min-h-[112px]">
            <div className="mb-2 flex items-center justify-between gap-3">
              <Hint className="m-0">{candidates.length ? "Follow from your list, or search for more above." : "What tx analyzes for you."}</Hint>
              {data.channels.length > 8 && (
                <Input
                  value={filter}
                  onChange={(e) => setFilter(e.target.value)}
                  placeholder={`Filter ${data.channels.length}`}
                  aria-label="Filter channels and shows"
                  className="h-8 w-44 px-2.5 text-[13px]"
                />
              )}
            </div>
            <div className="thin min-h-24 flex-1 overflow-y-auto rounded-lg ring-1 ring-foreground/10">
              {watching.length > 0 && (
                <>
                  {section("Following", watching.length)}
                  <ul className="m-0 list-none p-0">
                    {shownWatching.map((c) => row(c, <FollowingMark busy={pending.has(c.id)} onStop={() => void unfollow(c)} />))}
                  </ul>
                </>
              )}
              {candidates.length > 0 && (
                <>
                  {section("Your subscriptions", candidates.length, pickTools)}
                  <ul className="m-0 list-none p-0">
                    {shownCandidates.map((c) =>
                      row(
                        c,
                        <Button variant="secondary" size="xs" className="w-[84px] shrink-0" disabled={pending.has(c.id)} onClick={() => void follow(c)}>
                          {pending.has(c.id) ? <Spinner className="size-3" /> : "Follow"}
                        </Button>
                      )
                    )}
                  </ul>
                </>
              )}
              {q && !shownWatching.length && !shownCandidates.length && (
                <p className="m-0 px-3.5 py-3 text-[13px] text-muted-foreground">Nothing in your list matches "{filter.trim()}". Search above to find it.</p>
              )}
            </div>
          </div>
        )}
      </CardContent>
      <CardFooter className="flex-wrap justify-between gap-2.5">
        <div className="flex flex-wrap items-center gap-2">
          {!watching.length && <span className="text-[13px] text-faint">Follow at least one to continue.</span>}
          {hasList && !showImport && (
            <Button variant="ghost" size="sm" className="text-muted-foreground" onClick={() => setShowImport(true)}>
              Import subscriptions
            </Button>
          )}
        </div>
        <Button className="ml-auto" disabled={!watching.length} onClick={() => ctx.setStep("daily")}>
          Continue
        </Button>
      </CardFooter>
    </>
  )
}
