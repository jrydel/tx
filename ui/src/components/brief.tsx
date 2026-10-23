import { useEffect, useMemo, useRef, useState, type ReactNode } from "react"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ExpandButton, ExtLink, GROUPS, Thumb, kindColor, kindOf, type Kind } from "@/components/bits"
import { cn } from "@/lib/utils"
import { plural, released, secs, today, type Locale } from "@/lib/format"
import { sendMessage, useHost } from "@/host"
import type { BriefData, Card } from "@/types"

const panel = "min-h-0 overflow-hidden rounded-xl border bg-card"
const label = "mt-[22px] mb-1.5 block text-[11px] font-bold tracking-[.07em] text-faint uppercase"

/** One video: shown in full. More: a list grouped Watch, Skim, Skip (videos arrive in that order) with the selected video beside it. */
export function Brief({ data }: { data: BriefData }) {
  const host = useHost()
  const loc: Locale = host
  const videos = data.videos
  const n = videos.length

  const scope = useMemo(() => {
    const channels = [...new Set(videos.map((v) => v.channel))]
    const episodes = videos.filter((v) => v.podcast).length
    const what = [n - episodes && plural(n - episodes, "video"), episodes && plural(episodes, "episode")].filter(Boolean).join(" and ")
    const sources = !episodes ? "channels" : episodes === n ? "shows" : "channels and shows"
    const from = channels.length > 2 ? `${channels.length} ${sources}` : channels.join(" and ")
    const times = videos.map((v) => Date.parse(v.published)).filter((t) => !isNaN(t))
    let when = ""
    if (times.length) {
      const a = released(Math.min(...times), loc, true)
      const b = released(Math.max(...times), loc, true)
      when = a === b ? `, released ${a}` : `, released ${a} – ${b}`
    }
    return `${what} from ${from}${when}`
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [videos, host.locale, host.timeZone])

  const [current, setCurrent] = useState(0)
  const [showing, setShowing] = useState(false) // narrow mode: the detail pane is up
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([])
  useEffect(() => {
    setCurrent(0)
    setShowing(false)
  }, [videos])

  const select = (i: number, show: boolean, focus = false) => {
    if (i < 0 || i >= n) return
    setCurrent(i)
    setShowing(show)
    const b = itemRefs.current[i]
    b?.scrollIntoView({ block: "nearest" })
    if (focus) b?.focus()
  }

  const head = (
    <header className="mx-0.5 mb-2.5 grid gap-px">
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1">
        <h1 className="m-0 text-[15px] font-bold tracking-[-.005em]">
          Your brief<span className="ml-2 font-medium text-muted-foreground">{today(loc)}</span>
        </h1>
        <div className="flex items-center gap-3">
          <span className="text-[12.5px] text-faint tabular-nums">{scope}</span>
          <ExpandButton />
        </div>
      </div>
      {data.profile && (
        <p className="m-0 truncate text-[12.5px] text-faint" title={data.profile}>
          <b className="mr-1 font-semibold text-muted-foreground">Scored for</b>
          {data.profile}
        </p>
      )}
    </header>
  )

  if (n === 1) {
    return (
      <>
        {head}
        <ScrollArea className={cn(panel)}>
          <div className="px-5 pt-[18px] pb-[22px]">
            <Detail v={videos[0]} />
          </div>
        </ScrollArea>
      </>
    )
  }

  let last: Kind | null = null
  const rows: ReactNode[] = []
  videos.forEach((v, i) => {
    const k = kindOf(v)
    if (k !== last) {
      last = k
      const count = videos.filter((x) => kindOf(x) === k).length
      rows.push(
        <div
          key={`g-${k}`}
          role="presentation"
          className={cn(
            "sticky top-0 z-10 flex items-center gap-[7px] bg-card px-2 pt-2.5 pb-1.5 text-[11px] font-bold tracking-[.07em] text-(--k) uppercase before:size-1.5 before:rounded-full before:bg-(--k)",
            kindColor[k]
          )}
        >
          {GROUPS[k]}
          <span className="font-semibold tracking-normal text-faint">{count}</span>
        </div>
      )
    }
    rows.push(
      <button
        key={v.id}
        ref={(el) => {
          itemRefs.current[i] = el
        }}
        type="button"
        role="option"
        aria-selected={i === current}
        title={v.title}
        onClick={() => select(i, true)}
        className={cn(
          "relative grid w-full grid-cols-[84px_1fr] items-start gap-2.5 rounded-lg px-2 py-[7px] text-left hover:bg-muted aria-selected:bg-muted",
          "before:absolute before:top-2.5 before:bottom-2.5 before:left-0 before:hidden before:w-[3px] before:rounded-sm before:bg-(--k) aria-selected:before:block",
          kindColor[k]
        )}
      >
        <Thumb v={v} className="rounded-[5px]" />
        <div className="min-w-0">
          <h3 className="m-0 line-clamp-2 text-[13px] leading-[1.32] font-semibold">{v.title}</h3>
          <div className="mt-0.5 text-[11.5px] text-faint tabular-nums">
            {v.channel} · {released(v.published, loc, true)}
            {v.analyzed && (
              <>
                {" · "}
                <b className="font-semibold text-muted-foreground">{v.relevance ?? 0}/5</b>
              </>
            )}
          </div>
        </div>
      </button>
    )
  })

  return (
    <>
      {head}
      <div
        className={cn(
          panel,
          "grid flex-1 grid-cols-[minmax(250px,min(34%,420px))_minmax(0,1fr)] grid-rows-[minmax(0,1fr)] max-[600px]:grid-cols-1"
        )}
      >
        <ScrollArea className={cn("min-h-0 border-r max-[600px]:border-r-0", showing && "max-[600px]:hidden")}>
          <div
            role="listbox"
            aria-label="Videos"
            className="px-1.5 pb-2"
            onKeyDown={(e) => {
              if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return
              e.preventDefault()
              select(current + (e.key === "ArrowDown" ? 1 : -1), false, true)
            }}
          >
            {rows}
          </div>
        </ScrollArea>
        <ScrollArea key={videos[current].id} className={cn("min-h-0", !showing && "max-[600px]:hidden")}>
          <div aria-live="polite" className="px-5 pt-[18px] pb-[22px]">
            <Button
              variant="ghost"
              size="sm"
              className="mb-3 hidden px-1.5 font-semibold text-muted-foreground max-[600px]:inline-flex"
              onClick={() => setShowing(false)}
            >
              ← All videos
            </Button>
            <Detail v={videos[current]} />
          </div>
        </ScrollArea>
      </div>
    </>
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
          <div
            className="absolute top-[9px] h-2 rounded bg-(--k)/40"
            style={{ left: pct(from), width: `calc(${pct(to)} - ${pct(from)})` }}
          />
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
              <TooltipTrigger asChild>
                {t.url ? <ExtLink url={t.url} tabIndex={-1} {...props} /> : <span {...props} />}
              </TooltipTrigger>
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

/** The selected video: title beside a small thumbnail, the verdict, the timeline with the watch advice, TL;DR, then what it means for the user. */
function Detail({ v }: { v: Card }) {
  const host = useHost()
  const [lit, setLit] = useState<number | null>(null)
  const [asked, setAsked] = useState<"idle" | "sending" | "sent" | "failed">("idle")
  const kind = kindOf(v)
  const r = v.relevance ?? 0
  const loc: Locale = host

  const hero = (
    <div className={cn("grid grid-cols-[188px_1fr] items-start gap-4 max-[760px]:grid-cols-[132px_1fr] max-[760px]:gap-3", kindColor[kind])}>
      <Thumb v={v} link />
      <div className="min-w-0">
        <h2 className="m-0 mb-1 text-[17px] leading-[1.3] font-bold tracking-[-.01em] max-[760px]:text-[15.5px]">{v.title}</h2>
        <div className="text-faint tabular-nums" title={v.published}>
          {v.channel} · {released(v.published, loc)}
        </div>
        {v.analyzed && (
          <div className="mt-2 inline-flex items-baseline gap-2 text-[13px] font-bold text-(--k)">
            {GROUPS[kind]}
            <span className="font-mono text-xs font-semibold text-muted-foreground" title={`${r} of 5 for you`}>
              {r}/5 for you
            </span>
          </div>
        )}
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
    return (
      <>
        {hero}
        <p className="mt-4 mb-0 text-faint">Not analyzed yet.</p>
        {host.canMessage && (
          <div className="mt-4 flex items-center gap-3">
            <Button disabled={asked === "sending" || asked === "sent"} onClick={ask}>
              Analyze this one
            </Button>
            {asked === "sent" && <span className="text-[13px] text-muted-foreground">Asked your AI. It'll show up in your brief.</span>}
            {asked === "failed" && <span role="alert" className="text-[13px] text-destructive">Couldn't reach your AI. Try again.</span>}
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
      {v.verdict && <p className="mt-4 mb-0 text-[15px] leading-normal text-foreground">{v.verdict}</p>}
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
              <li
                key={i}
                onMouseEnter={() => setLit(i)}
                onMouseLeave={() => setLit(null)}
                className="grid grid-cols-[54px_1fr] items-baseline gap-2.5 rounded-md"
              >
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
