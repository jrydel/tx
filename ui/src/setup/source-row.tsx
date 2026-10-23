import { useEffect, useRef, useState } from "react"
import { CheckIcon } from "lucide-react"
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"
import { callTool } from "@/host"
import type { Channel } from "@/types"

export interface Source {
  title: string
  kind: "youtube" | "podcast"
  image?: string | null
  subtitle?: string | null
  /** A line under the subtitle, e.g. why the AI picked it. */
  note?: string | null
  /** Details are on their way: placeholders instead of the avatar and subtitle. */
  loading?: boolean
}

/**
 * A channel or show the way it appears everywhere in setup (search results, what you follow, your subscriptions):
 * avatar, name, "@handle · subscribers", platform. Lay it out in a flex row; the action goes after it.
 */
export function SourceLine({ s, pick }: { s: Source; pick?: boolean }) {
  const round = s.kind === "youtube"
  return (
    <>
      {s.loading && !s.image ? (
        <span aria-hidden className={cn("size-9 shrink-0 animate-pulse bg-foreground/10", round ? "rounded-full" : "rounded-lg")} />
      ) : (
        <Avatar className={cn("size-9 shrink-0 rounded-lg after:rounded-lg", round && "rounded-full after:rounded-full")}>
          {s.image && <AvatarImage src={s.image} alt="" className={round ? "" : "rounded-lg"} />}
          <AvatarFallback className={round ? "" : "rounded-lg"}>{s.title.trim().charAt(0).toUpperCase()}</AvatarFallback>
        </Avatar>
      )}
      <div className="min-w-0 flex-1">
        <div className="truncate text-[14px] leading-tight font-semibold">
          {pick && <span className="mr-1.5 text-[11px] font-bold tracking-[.05em] text-watch uppercase">Pick</span>}
          {s.title}
        </div>
        {s.loading && !s.subtitle ? (
          <span aria-label="Loading details" className="mt-1.5 block h-2.5 w-40 max-w-full animate-pulse rounded-full bg-foreground/10" />
        ) : (
          s.subtitle && <div className="mt-0.5 truncate text-[12.5px] text-muted-foreground">{s.subtitle}</div>
        )}
        {s.note && <div className="mt-0.5 text-[12.5px] text-muted-foreground">{s.note}</div>}
      </div>
      <Badge variant="outline" className="shrink-0 text-faint">
        {s.kind === "podcast" ? "Podcast" : "YouTube"}
      </Badge>
    </>
  )
}

/** "✓ Following"; with `onStop`, hovering it offers to stop. */
export function FollowingMark({ onStop, busy }: { onStop?: () => void; busy?: boolean }) {
  const label = (
    <>
      <CheckIcon className="size-3.5" />
      Following
    </>
  )
  if (!onStop) return <span className="inline-flex w-[84px] shrink-0 items-center justify-end gap-1 text-xs font-semibold text-watch">{label}</span>
  return (
    <button
      type="button"
      onClick={onStop}
      disabled={busy}
      className="group/stop inline-flex h-7 w-[84px] shrink-0 cursor-pointer items-center justify-center gap-1 rounded-md text-xs font-semibold text-watch hover:bg-destructive/10 hover:text-destructive disabled:opacity-60"
    >
      <span className="inline-flex items-center gap-1 group-hover/stop:hidden">{label}</span>
      <span className="hidden group-hover/stop:inline">Unfollow</span>
    </button>
  )
}

/** Channels per channel_details call: small enough that rows fill in as they arrive (tx takes about 0.1 s each). */
const BATCH = 10

/**
 * Avatars and "@handle · subscribers" for channels tx doesn't have them for yet, asked from tx in batches (the ones
 * listed first first). Returns how to show a channel as a row, placeholders included while its details are on the way.
 */
export function useChannelDetails(channels: Channel[], canTools: boolean) {
  const [found, setFound] = useState<ReadonlyMap<string, { thumbnail: string | null; subtitle: string | null }>>(new Map())
  const [waiting, setWaiting] = useState<ReadonlySet<string>>(new Set())
  const asked = useRef(new Set<string>())
  // Batches run to the end even when the list refreshes (a follow moves a row): only leaving the step stops them.
  const alive = useRef(true)
  useEffect(() => {
    alive.current = true
    return () => {
      alive.current = false
    }
  }, [])

  useEffect(() => {
    if (!canTools) return
    const missing = channels
      .filter((c) => !c.thumbnail && !c.subtitle && !asked.current.has(c.id))
      .sort((a, b) => Number(b.watching) - Number(a.watching) || Number(!!b.suggested) - Number(!!a.suggested))
      .map((c) => c.id)
    if (!missing.length) return
    for (const id of missing) asked.current.add(id)
    void (async () => {
      setWaiting((w) => new Set([...w, ...missing]))
      for (let i = 0; i < missing.length && alive.current; i += BATCH) {
        const ids = missing.slice(i, i + BATCH)
        try {
          const r = await callTool("channel_details", { ids })
          const got = (r.structuredContent as { channels?: { id: string; thumbnail: string | null; subtitle: string | null }[] } | undefined)?.channels ?? []
          if (alive.current) setFound((m) => new Map([...m, ...got.map((g) => [g.id, { thumbnail: g.thumbnail, subtitle: g.subtitle }] as const)]))
        } catch {
          /* no details: the initial and the name stay */
        }
        if (alive.current)
          setWaiting((w) => {
            const n = new Set(w)
            for (const id of ids) n.delete(id)
            return n
          })
      }
    })()
  }, [channels, canTools])

  /** The channel as a row: what tx sent, else what was fetched since, else placeholders while it's on its way. */
  return (c: Channel): Source => {
    const f = found.get(c.id)
    return {
      title: c.title,
      kind: c.podcast ? "podcast" : "youtube",
      image: c.thumbnail ?? f?.thumbnail ?? null,
      subtitle: c.subtitle ?? f?.subtitle ?? null,
      note: c.why,
      loading: waiting.has(c.id),
    }
  }
}
