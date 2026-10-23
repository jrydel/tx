import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import { ArrowLeftIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ExpandButton, ExtLink, GROUPS, Thumb, kindColor, kindOf, type Kind } from "@/components/bits"
import { Landing } from "@/components/landing"
import { Show, reelSeconds, showOf } from "@/components/show"
import { cn } from "@/lib/utils"
import { released, secs, type Locale } from "@/lib/format"
import { sendMessage, useHost } from "@/host"
import type { BriefData, Card } from "@/types"

const label = "mt-6 mb-2 block text-[11px] font-bold tracking-[.08em] text-faint uppercase"

/** What's open over the landing: a video's page, or the narrated show. */
type Open = { kind: "video"; id: string } | { kind: "show" }

/**
 * The brief: a landing page for the period (or the list of videos it was asked for), a page per video, and the narrated
 * show. One video is shown as its page straight away. The frame scrolls inside; coming back lands where you were.
 */
export function Brief({ data }: { data: BriefData }) {
  const host = useHost()
  const loc: Locale = host
  const videos = data.videos
  // What's open belongs to the result it was opened in: a new tool result shows its landing again.
  const [opened, setOpened] = useState<{ in: Card[]; what: Open } | null>(null)
  const what = opened?.in === videos ? opened.what : null
  const scroller = useRef<HTMLDivElement>(null)
  const landingTop = useRef(0)
  const show = useMemo(() => showOf(data), [data])

  const go = (w: Open | null) => {
    if (w && !what) landingTop.current = scroller.current?.scrollTop ?? 0
    setOpened(w ? { in: videos, what: w } : null)
  }
  useLayoutEffect(() => {
    scroller.current?.scrollTo(0, what ? 0 : landingTop.current)
  }, [what])
  useEffect(() => {
    if (!what) return
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !e.defaultPrevented && setOpened(null)
    addEventListener("keydown", onKey)
    return () => removeEventListener("keydown", onKey)
  }, [what])

  const single = videos.length === 1
  const video = single ? videos[0] : what?.kind === "video" ? videos.find((v) => v.id === what.id) : undefined
  return (
    <div ref={scroller} className="thin @container min-h-0 flex-1 overflow-y-auto rounded-xl border bg-card">
      {what?.kind === "show" && show ? (
        <Show show={show} loc={loc} onClose={() => go(null)} />
      ) : video ? (
        <div className="mx-auto max-w-[860px] px-4 pt-3 pb-8 @min-[900px]:px-6">
          <div className="mb-4 flex items-center justify-between gap-3">
            {single ? (
              <span className="font-slate text-[17px]">Your brief</span>
            ) : (
              <Button variant="ghost" size="sm" className="-ml-2.5 font-semibold text-muted-foreground" onClick={() => go(null)}>
                <ArrowLeftIcon data-icon="inline-start" />
                Your brief
              </Button>
            )}
            <ExpandButton />
          </div>
          <Detail v={video} loc={loc} />
        </div>
      ) : (
        <Landing
          data={data}
          loc={loc}
          actions={<ExpandButton />}
          open={(id) => go({ kind: "video", id })}
          watch={show ? { seconds: reelSeconds(show), videos: show.clips.length, start: () => go({ kind: "show" }) } : undefined}
        />
      )}
    </div>
  )
}

/** A timestamp chip that plays the video from that moment. */
function Stamp({ at, url, lit, onLit }: { at: string; url?: string; lit?: boolean; onLit?: (on: boolean) => void }) {
  const cls = cn(
    "justify-self-start rounded-[5px] bg-muted px-1.5 font-mono text-xs leading-[1.65] font-semibold whitespace-nowrap text-foreground",
    url && "hover:bg-foreground hover:text-card",
    lit && "bg-foreground text-card"
  )
  const hover = { onMouseEnter: () => onLit?.(true), onMouseLeave: () => onLit?.(false), onFocus: () => onLit?.(true), onBlur: () => onLit?.(false) }
  return url ? (
    <ExtLink url={url} className={cls} title={`Play from ${at}`} {...hover}>
      {at}
    </ExtLink>
  ) : (
    <span className={cls}>{at}</span>
  )
}

/** Where the value sits in the video: the part to watch shaded, takeaways as ticks that play from that moment. */
function Timeline({ v, kind, lit, setLit }: { v: Card; kind: Kind; lit: number | null; setLit: (i: number | null) => void }) {
  const total = secs(v.length)
  if (!total) return null
  const pct = (s: number) => `${Math.min(100, Math.max(0, (s / total) * 100))}%`
  const from = kind === "watch" ? 0 : secs(v.skim_from)
  const to = kind === "watch" ? total : secs(v.skim_to)
  return (
    <div className="mt-4 mb-1" aria-hidden>
      <div className="relative h-[26px]">
        <div className="absolute inset-x-0 top-3 h-0.5 rounded-[1px] bg-border" />
        {kind !== "skip" && from != null && to != null && to > from && (
          <div className="absolute top-[9px] h-2 rounded bg-(--k)/40" style={{ left: pct(from), width: `calc(${pct(to)} - ${pct(from)})` }} />
        )}
        {(v.takeaways ?? []).map((t, i) => {
          const s = secs(t.at)
          if (s == null) return null
          const cls = cn(
            "group/tick absolute top-[5px] -ml-2 grid size-4 place-items-center rounded-full",
            "after:size-2 after:rounded-full after:border-2 after:border-foreground after:bg-card after:transition-[transform,background-color] after:duration-150",
            "hover:after:scale-[1.35] hover:after:bg-foreground",
            lit === i && "after:scale-[1.35] after:bg-foreground"
          )
          const props = { className: cls, style: { left: pct(s) }, onMouseEnter: () => setLit(i), onMouseLeave: () => setLit(null) }
          return (
            <Tooltip key={i}>
              <TooltipTrigger asChild>{t.url ? <ExtLink url={t.url} tabIndex={-1} {...props} /> : <span {...props} />}</TooltipTrigger>
              <TooltipContent className="max-w-64">
                {t.at} · {t.text}
              </TooltipContent>
            </Tooltip>
          )
        })}
      </div>
      <div className="flex justify-between font-mono text-[10.5px] font-medium text-faint">
        <span>0:00</span>
        <span>{v.length}</span>
      </div>
    </div>
  )
}

/** One video, to dig into: the slate (thumbnail, title, verdict), the timeline with the watch advice, what it means for you, what it says. */
function Detail({ v, loc }: { v: Card; loc: Locale }) {
  const host = useHost()
  const [lit, setLit] = useState<number | null>(null)
  const [asked, setAsked] = useState<"idle" | "sending" | "sent" | "failed">("idle")
  const kind = kindOf(v)
  const r = v.relevance ?? 0

  const hero = (
    <div className={cn("grid grid-cols-[minmax(0,1fr)] items-end gap-4 @min-[640px]:grid-cols-[minmax(0,260px)_1fr] @min-[640px]:gap-5", kindColor[kind])}>
      <Thumb v={v} link />
      <div className="min-w-0">
        <div className="mb-1.5 inline-flex items-baseline gap-2 text-[12.5px] font-bold tracking-[.05em] text-(--k) uppercase">
          {GROUPS[kind]}
          {v.analyzed && (
            <span className="font-mono text-xs font-semibold tracking-normal text-muted-foreground normal-case" title={`${r} of 5 for you`}>
              {r}/5 for you
            </span>
          )}
        </div>
        <h2 className="m-0 mb-1.5 font-slate text-[27px] leading-[1.03] text-balance @min-[640px]:text-[32px]">{v.title}</h2>
        <div className="text-[13px] text-muted-foreground tabular-nums" title={v.published}>
          {v.channel} · {released(v.published, loc)}
          {v.length && <> · {v.length}</>}
        </div>
      </div>
    </div>
  )

  if (!v.analyzed) {
    const ask = async () => {
      setAsked("sending")
      try {
        await sendMessage(`Use tx: analyze "${v.title}" (${v.id}) for me and show it in my brief.`)
        setAsked("sent")
      } catch {
        setAsked("failed")
      }
    }
    const why = kind === "failed" ? `Couldn't analyze this one: ${v.error || "no transcript to read."}` : v.state === "passed" ? `Passed on from its title and notes: ${v.error || "not for you."}` : "Not scored yet. Your AI scores it on the next screening."
    return (
      <>
        {hero}
        <p className={cn("mt-5 mb-0", kind === "failed" ? "text-destructive" : "text-muted-foreground")}>{why}</p>
        {host.canMessage && (
          <div className="mt-4 flex items-center gap-3">
            <Button disabled={asked === "sending" || asked === "sent"} onClick={ask}>
              {kind === "failed" ? "Try again" : "Analyze this one"}
            </Button>
            {asked === "sent" && <span className="text-[13px] text-muted-foreground">Asked your AI. It'll show up in your brief.</span>}
            {asked === "failed" && (
              <span role="alert" className="text-[13px] text-destructive">
                Couldn't reach your AI. Try again.
              </span>
            )}
          </div>
        )}
      </>
    )
  }

  const range = kind === "skim" && v.skim_from ? (v.skim_to ? `${v.skim_from}–${v.skim_to}` : v.skim_from) : null
  const showTimeline = !!secs(v.length)
  const takeaways = v.takeaways ?? []
  return (
    <>
      {hero}
      {v.verdict && <p className="mt-5 mb-0 text-[16px] leading-[1.45] text-pretty text-foreground">{v.verdict}</p>}
      {(showTimeline || v.reason) && (
        <div className={kindColor[kind]}>
          <Timeline v={v} kind={kind} lit={lit} setLit={setLit} />
          {v.reason && (
            <p className="mt-1.5 mb-0 text-[13px] text-muted-foreground">
              {/* The advice itself is already in the header; only a skim range adds to it here. */}
              {range && (
                <>
                  <b className="mr-1 font-[650] text-(--k)">Skim</b>
                  <Stamp at={range} url={v.skim_url} />{" "}
                </>
              )}
              {v.reason}
            </p>
          )}
        </div>
      )}
      {takeaways.length > 0 && (
        <>
          <span className={label}>For you</span>
          <ul className="m-0 grid list-none gap-2.5 p-0">
            {takeaways.map((t, i) => (
              <li key={i} onMouseEnter={() => setLit(i)} onMouseLeave={() => setLit(null)} className="grid grid-cols-[54px_1fr] items-baseline gap-2.5 rounded-md">
                {t.at ? <Stamp at={t.at} url={t.url} lit={lit === i} onLit={(on) => setLit(on ? i : null)} /> : <span />}
                <span>{t.text}</span>
              </li>
            ))}
          </ul>
        </>
      )}
      {v.tldr && (
        <>
          <span className={label}>What it says</span>
          <p className="m-0 text-muted-foreground">{v.tldr}</p>
        </>
      )}
    </>
  )
}
