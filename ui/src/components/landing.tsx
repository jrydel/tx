import { memo, type CSSProperties, type ReactNode } from "react"
import { PlayIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ExtLink, GROUPS, Thumb, kindColor, kindOf, type Kind } from "@/components/bits"
import { cn } from "@/lib/utils"
import { plural, released, secs, type Locale } from "@/lib/format"
import type { BriefData, Card, Debrief } from "@/types"

const eyebrow = "m-0 text-[11px] font-bold tracking-[.09em] text-faint uppercase"

/** "1 h 12 min", "38 min". */
export function span(s: number): string {
  const m = Math.round(s / 60)
  if (m < 60) return `${m} min`
  return m % 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m / 60} h`
}

/** The stretch of a video worth watching, in seconds: all of a Watch, the range of a Skim, none of the rest. */
export function worth(v: Card): [number, number] | null {
  const total = secs(v.length)
  if (!total) return null
  const k = kindOf(v)
  if (k === "watch") return [0, total]
  if (k !== "skim") return null
  const from = secs(v.skim_from)
  const to = secs(v.skim_to)
  return from != null && to != null && to > from ? [from, Math.min(to, total)] : null
}

export interface LandingProps {
  data: BriefData
  loc: Locale
  /** Header controls (full screen). */
  actions: ReactNode
  open: (id: string) => void
  /** The reel over the picks, when there's anything to play. */
  watch?: { seconds: number; videos: number; start: () => void }
}

/**
 * The brief as a landing page: how much of the period is worth your time (the reel), what it adds up to (the debrief
 * your AI wrote), the picks, and everything else in a quieter column. Every video opens its own page.
 */
export function Landing({ data, loc, actions, open, watch }: LandingProps) {
  const videos = data.videos
  const of = (k: Kind) => videos.filter((v) => kindOf(v) === k)
  const picks = videos.filter((v) => kindOf(v) === "watch" || kindOf(v) === "skim")

  return (
    <div className="grid grid-cols-[minmax(0,1fr)] gap-x-10 px-4 pt-3 pb-8 @min-[900px]:grid-cols-[minmax(0,1fr)_280px] @min-[900px]:px-6">
      <header className="col-span-full flex items-start justify-between gap-4">
        <div className="min-w-0">
          <span className="font-slate text-[17px]">Your brief</span>
          {data.profile && (
            <p className="m-0 max-w-[60ch] truncate text-[12.5px] text-faint" title={data.profile}>
              Scored for: {data.profile}
            </p>
          )}
        </div>
        {actions}
      </header>

      <section aria-labelledby="cut" className="col-span-full mt-5 mb-8 @min-[900px]:mt-7 @min-[900px]:mb-11">
        <Reel videos={videos} days={data.days} loc={loc} open={open} />
        {watch && (
          <div className="mt-5 flex flex-wrap items-center gap-x-3 gap-y-1.5">
            <Button onClick={watch.start}>
              <PlayIcon data-icon="inline-start" />
              Watch your debrief
            </Button>
            <span className="text-[12.5px] text-muted-foreground tabular-nums">
              {watch.seconds < 60 ? `${Math.round(watch.seconds)} s` : `${Math.floor(watch.seconds / 60)} min ${Math.round(watch.seconds % 60)} s`} · {plural(watch.videos, "video")}
            </span>
          </div>
        )}
      </section>

      <main className="min-w-0">
        <DebriefSection debrief={data.debrief} videos={videos} days={data.days} open={open} />
        {picks.length > 0 && (
          <section aria-labelledby="picks" className="mt-10 first:mt-0">
            <h2 id="picks" className={eyebrow}>
              Worth your time
            </h2>
            <div className="mt-3.5 grid gap-2.5">
              {picks.map((v) => (
                <Pick key={v.id} v={v} loc={loc} open={open} />
              ))}
            </div>
          </section>
        )}
      </main>

      <aside className="min-w-0 @max-[899px]:mt-10" aria-label="Everything else">
        <Rail title="Skipped" hint="Scored, and not for you." videos={of("skip")} loc={loc} open={open} why />
        <Rail title="Not scored yet" hint="Your AI scores them on the next screening." videos={of("new")} loc={loc} open={open} />
        <Rail title="Couldn't analyze" videos={of("failed")} loc={loc} open={open} why />
      </aside>
    </div>
  )
}

/**
 * The signature: the uploads as one strip, oldest left, each as long as the video. The stretches worth watching are lit
 * (all of a Watch, the range of a Skim); unscored footage is hatched; the rest stays dark.
 */
function Reel({ videos, days, loc, open }: { videos: Card[]; days?: number; loc: Locale; open: (id: string) => void }) {
  const strip = [...videos].sort((a, b) => Date.parse(a.published) - Date.parse(b.published))
  const known = strip.map((c) => secs(c.length)).filter((s): s is number => !!s)
  const typical = known.length ? known.reduce((a, b) => a + b, 0) / known.length : 600
  const footage = known.reduce((a, b) => a + b, 0)
  const lit = strip.reduce((a, c) => {
    const w = worth(c)
    return a + (w ? w[1] - w[0] : 0)
  }, 0)
  const scored = strip.filter((c) => c.analyzed).length
  const channels = new Set(strip.map((c) => c.channel)).size
  const unknown = strip.length - known.length
  const picks = strip.filter((c) => kindOf(c) === "watch" || kindOf(c) === "skim")
  // Minutes only when they add up: every scored upload has a length and every pick says which stretch to watch.
  // Unscored uploads without one are left out of the total (the legend says how many).
  const timed = strip.every((c) => !c.analyzed || secs(c.length)) && picks.every((c) => worth(c))
  const first = released(strip[0]!.published, loc, true)
  const last = released(strip.at(-1)!.published, loc, true)
  const legend: [Kind, string][] = [
    ["watch", "Watch"],
    ["skim", "Skim"],
    ["skip", "Skip"],
    ["new", "Not scored"],
    ["failed", "Couldn't analyze"],
  ]

  return (
    <>
      <p className={eyebrow}>
        {days ? `The last ${days} days` : first === last ? first : `${first}–${last}`} · {plural(channels, "channel")} · {plural(strip.length, "upload")}
      </p>
      <h1 id="cut" className="m-0 mt-2.5 max-w-[22ch] font-slate text-[36px] leading-[.97] text-balance @min-[640px]:text-[44px] @min-[900px]:text-[54px]">
        {scored === 0 ? (
          <>
            {timed ? `${span(footage)} of new video.` : `${plural(strip.length, "new upload")}.`} <span className="text-muted-foreground">None of it scored yet.</span>
          </>
        ) : picks.length === 0 ? (
          <>None of these {timed ? span(footage) : plural(strip.length, "upload")} is worth your time.</>
        ) : timed ? (
          <>
            {span(lit)} <span className="text-muted-foreground">of {span(footage)} is worth your time.</span>
          </>
        ) : (
          <>
            {picks.length} <span className="text-muted-foreground">of {plural(strip.length, "upload")} {picks.length === 1 ? "is" : "are"} worth your time.</span>
          </>
        )}
      </h1>

      <div className="mt-5 flex h-10 items-stretch gap-[3px] @min-[900px]:mt-7 @min-[900px]:h-12" role="list" aria-label="The uploads, oldest first">
        {strip.map((c, i) => (
          <Segment key={c.id} v={c} grow={secs(c.length) ?? typical} delay={Math.min(i * 28, 700)} loc={loc} open={open} />
        ))}
      </div>
      <div className="mt-1.5 flex justify-between font-mono text-[10.5px] text-faint">
        <span>{first}</span>
        <span>{last}</span>
      </div>

      <ul className="m-0 mt-3 flex list-none flex-wrap gap-x-5 gap-y-1 p-0 text-[12.5px] text-muted-foreground">
        {legend.map(([k, name]) => {
          const these = strip.filter((c) => kindOf(c) === k)
          if (!these.length) return null
          const time = these.reduce((a, c) => {
            const w = worth(c)
            return a + (w ? w[1] - w[0] : 0)
          }, 0)
          return (
            <li key={k} className={cn("flex items-center gap-2", kindColor[k])}>
              <span
                aria-hidden
                className={cn("size-2.5 rounded-[2px]", k === "new" ? "reel-unseen bg-muted" : k === "skip" ? "bg-muted ring-1 ring-border" : k === "failed" ? "bg-(--k)/40" : "bg-(--k)")}
              />
              {name} {these.length}
              {timed && time > 0 && <span className="font-mono text-[11.5px] text-faint">{span(time)}</span>}
            </li>
          )
        })}
        {unknown > 0 && <li className="text-faint">{plural(unknown, "upload")} without a length, drawn at a typical length</li>}
      </ul>
    </>
  )
}

/** One upload on the reel. */
const Segment = memo(function Segment({ v, grow, delay, loc, open }: { v: Card; grow: number; delay: number; loc: Locale; open: (id: string) => void }) {
  const k = kindOf(v)
  const total = secs(v.length)
  const w = worth(v)
  const style: CSSProperties = { flexGrow: grow, flexBasis: 0 }
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          role="listitem"
          onClick={() => open(v.id)}
          aria-label={`${v.title}, ${v.channel}: ${GROUPS[k]}`}
          style={style}
          className={cn(
            "relative min-w-[5px] cursor-pointer overflow-hidden rounded-[3px] bg-muted transition-[filter] hover:brightness-125",
            k === "new" && "reel-unseen",
            k === "failed" && "bg-(--k)/25",
            kindColor[k]
          )}
        >
          {w && total && (
            <span
              aria-hidden
              className="reel-lit absolute inset-y-0 bg-(--k)"
              style={{ left: `${(w[0] / total) * 100}%`, width: `${((w[1] - w[0]) / total) * 100}%`, animationDelay: `${delay}ms` }}
            />
          )}
          {k === "skim" && !w && <span aria-hidden className="reel-lit absolute inset-0 bg-(--k)/35" style={{ animationDelay: `${delay}ms` }} />}
        </button>
      </TooltipTrigger>
      <TooltipContent className={cn("max-w-72 flex-col items-start gap-0.5", kindColor[k])}>
        <b className="block font-semibold">{v.title}</b>
        <span className="opacity-75">
          {v.channel} · {released(v.published, loc, true)}
          {v.length && ` · ${v.length}`} · {GROUPS[k]}
          {k === "skim" && v.skim_from && ` ${v.skim_from}${v.skim_to ? `–${v.skim_to}` : ""}`}
        </span>
      </TooltipContent>
    </Tooltip>
  )
})

/** What the period adds up to, as your AI wrote it after the last screening. */
function DebriefSection({ debrief, videos, days, open }: { debrief?: Debrief | null; videos: Card[]; days?: number; open: (id: string) => void }) {
  const analyzed = videos.filter((v) => v.analyzed)
  if (!debrief) {
    // Only the week's brief promises one; a list of picked videos has none to wait for.
    if (!days || analyzed.length < 2) return null
    return (
      <section aria-labelledby="debrief">
        <h2 id="debrief" className={eyebrow}>
          Debrief
        </h2>
        <p className="m-0 mt-2.5 text-[14.5px] text-muted-foreground">Your AI writes it at the end of the next screening, across everything it scored this week.</p>
      </section>
    )
  }
  const byId = new Map(videos.map((v) => [v.id, v]))
  const covered = new Set(debrief.video_ids)
  const newer = analyzed.filter((v) => !covered.has(v.id)).length
  return (
    <section aria-labelledby="debrief">
      <h2 id="debrief" className={eyebrow}>
        Debrief
      </h2>
      <p className="m-0 mt-2.5 text-[20px] leading-[1.3] font-semibold tracking-[-.01em] text-pretty @min-[900px]:text-[22px]">{debrief.headline}</p>
      <p className="m-0 mt-2.5 max-w-[68ch] text-[14.5px] leading-[1.6] text-muted-foreground">{debrief.summary}</p>
      {debrief.threads.length > 0 && (
        <ul className="m-0 mt-5 grid list-none gap-4 p-0">
          {debrief.threads.map((t, i) => {
            const cited = t.video_ids.map((id) => byId.get(id)).filter((c): c is Card => !!c)
            return (
              <li key={i} className="grid gap-1 border-l-2 border-border pl-3.5">
                <b className="text-[14px] font-semibold">{t.title}</b>
                <p className="m-0 max-w-[66ch] text-[13.5px] leading-[1.55] text-muted-foreground">{t.text}</p>
                {cited.length > 0 && (
                  <div className="mt-1 flex flex-wrap gap-1.5">
                    {cited.map((c) => (
                      <button
                        key={c.id}
                        type="button"
                        onClick={() => open(c.id)}
                        className={cn("inline-flex max-w-64 items-center gap-2 rounded-md bg-muted py-1 pr-2.5 pl-1 text-left text-[12.5px] hover:bg-border", kindColor[kindOf(c)])}
                      >
                        <Thumb v={c} small className="w-11 shrink-0 rounded-[3px]" />
                        <span className="truncate">{c.title}</span>
                      </button>
                    ))}
                  </div>
                )}
              </li>
            )
          })}
        </ul>
      )}
      {newer > 0 && <p className="m-0 mt-3.5 text-[12.5px] text-faint">Written before {plural(newer, "newer video")}; your AI rewrites it at the next screening.</p>}
    </section>
  )
}

/** A pick: thumbnail, title, why it's for you, and where to start. */
const Pick = memo(function Pick({ v, loc, open }: { v: Card; loc: Locale; open: (id: string) => void }) {
  const k = kindOf(v)
  const w = worth(v)
  const total = secs(v.length)
  const watch = k === "watch"
  return (
    <article
      className={cn(
        "relative grid gap-3.5 rounded-xl border bg-card p-2.5 pr-3.5",
        watch ? "grid-cols-[136px_1fr] @min-[640px]:grid-cols-[184px_1fr]" : "grid-cols-[104px_1fr] @min-[640px]:grid-cols-[128px_1fr]",
        kindColor[k]
      )}
    >
      <Thumb v={v} small={!watch} className="self-start rounded-md" />
      <div className="min-w-0">
        <div className="flex flex-wrap items-baseline gap-x-2 text-[11.5px] font-bold tracking-[.06em] text-(--k) uppercase">
          {GROUPS[k]}
          {!watch && v.skim_from && (
            <span className="font-mono font-semibold tracking-normal normal-case">
              {v.skim_from}
              {v.skim_to && `–${v.skim_to}`}
              {w && ` · ${span(w[1] - w[0])}`}
            </span>
          )}
          <span className="font-mono font-semibold tracking-normal text-faint normal-case">{v.relevance ?? 0}/5</span>
        </div>
        <h3 className="m-0 mt-0.5 text-[15px] leading-[1.3] font-semibold text-pretty">
          {/* The whole card opens the video's page; links inside stay clickable above it. */}
          <button type="button" onClick={() => open(v.id)} className="text-left after:absolute after:inset-0 after:rounded-xl after:content-['']">
            {v.title}
          </button>
        </h3>
        <div className="mt-0.5 text-[12px] text-faint tabular-nums">
          {v.channel} · {released(v.published, loc, true)}
        </div>
        {v.verdict && <p className="m-0 mt-1.5 line-clamp-2 text-[13.5px] leading-[1.5] text-muted-foreground">{v.verdict}</p>}
        {!watch && total && w && (
          <div aria-hidden className="mt-2.5 h-1 overflow-hidden rounded-full bg-muted">
            <div className="h-full rounded-full bg-(--k)" style={{ marginLeft: `${(w[0] / total) * 100}%`, width: `${((w[1] - w[0]) / total) * 100}%` }} />
          </div>
        )}
        {!watch && v.skim_from && v.skim_url && (
          <ExtLink url={v.skim_url} className="relative z-10 mt-1.5 inline-block text-[12.5px] font-semibold text-(--k) hover:underline">
            Play from {v.skim_from}
          </ExtLink>
        )}
      </div>
    </article>
  )
})

/** A quiet list in the side column, for what the brief doesn't recommend. */
function Rail({ title, hint, videos, loc, open, why }: { title: string; hint?: string; videos: Card[]; loc: Locale; open: (id: string) => void; why?: boolean }) {
  if (!videos.length) return null
  return (
    <section className="mt-8 first:mt-0">
      <h2 className={eyebrow}>
        {title} · {videos.length}
      </h2>
      {hint && <p className="m-0 mt-1 text-[12.5px] text-faint">{hint}</p>}
      <ul className="m-0 mt-2 grid list-none gap-0.5 p-0">
        {videos.map((v) => (
          <Line key={v.id} v={v} loc={loc} open={open} why={why} />
        ))}
      </ul>
    </section>
  )
}

const Line = memo(function Line({ v, loc, open, why }: { v: Card; loc: Locale; open: (id: string) => void; why?: boolean }) {
  const note = why ? (v.analyzed ? v.verdict : v.error) : null
  return (
    <li>
      <button type="button" onClick={() => open(v.id)} className={cn("grid w-full grid-cols-[56px_1fr] gap-2.5 rounded-lg px-1.5 py-1.5 text-left hover:bg-muted", kindColor[kindOf(v)])}>
        <Thumb v={v} small className="rounded-[4px]" />
        <span className="min-w-0">
          <span className="line-clamp-2 text-[13px] leading-[1.3] font-medium">{v.title}</span>
          <span className="mt-0.5 block truncate text-[11.5px] text-faint">
            {v.channel} · {released(v.published, loc, true)}
          </span>
          {note && <span className="mt-0.5 line-clamp-2 text-[12px] leading-[1.4] text-muted-foreground">{note}</span>}
        </span>
      </button>
    </li>
  )
})
