import { useEffect, useMemo, useRef, useState } from "react"
import { ArrowLeftIcon, PauseIcon, PlayIcon, RotateCcwIcon, SkipBackIcon, SkipForwardIcon, Volume2Icon, VolumeXIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { ExpandButton, ExtLink, Thumb, kindOf } from "@/components/bits"
import { cn } from "@/lib/utils"
import { plural, secs } from "@/lib/format"
import { callTool } from "@/host"
import type { BriefData, Card } from "@/types"

/** One key part of a video: the narrator's setup, then the stretch where it's said, played as it is. */
export interface Part {
  say: string
  moment: [number, number]
}

/** One channel's chapter: the video, the narrator's takeaway, its key parts in order. */
export interface Chapter {
  video: Card
  intro: string
  parts: Part[]
}

export interface ShowScript {
  intro: string
  chapters: Chapter[]
  outro?: string | null
  /** Written by the user's AI; false when made up here from the analyses' TL;DRs and takeaways. */
  written: boolean
}

/** Channels in a debrief by default, and at most. */
export const MAX_CHANNELS = 5
/** A key part made from a takeaway: this long from its timestamp. */
const PART = 12
/** Under the narrator the montage's cuts run this fast, muted. */
const FAST = 2
/** About how long each cut of the montage is on screen. */
const CUT_MS = 1600
/** Sound fades in this long when a part starts, and out this long when it ends. */
const FADE_IN = 500
const FADE_OUT = 250

const firstSentence = (t: string) => t.split(/(?<=[.!?])\s/)[0]!.trim()
const clip = (t: string, n: number) => (t.length > n ? `${t.slice(0, n - 1).trimEnd()}…` : t)

/**
 * Every channel's chapter the brief can offer: your AI's script first, in its order, then a chapter made from the
 * analysis for each other channel with a pick (its best video: the TL;DR as the takeaway, the timestamped takeaways as
 * the key parts). YouTube only: tx streams YouTube videos; podcast audio isn't part of it.
 */
export function showOf(data: BriefData): ShowScript | null {
  const byId = new Map(data.videos.map((v) => [v.id, v]))
  const script = data.debrief?.script
  const chapters: Chapter[] = []
  const channels = new Set<string>()
  for (const s of script?.segments ?? []) {
    const video = byId.get(s.video_id)
    if (!video || video.podcast || channels.has(video.channel)) continue
    channels.add(video.channel)
    chapters.push({ video, intro: s.intro, parts: s.parts.map((p) => ({ say: p.say, moment: [p.moment_start, p.moment_end] })) })
  }
  const written = chapters.length > 0
  const best = data.videos
    .filter((v) => !v.podcast && v.analyzed && (kindOf(v) === "watch" || kindOf(v) === "skim"))
    .sort((a, b) => (b.relevance ?? 0) - (a.relevance ?? 0))
  for (const v of best) {
    if (channels.has(v.channel)) continue
    const total = secs(v.length) ?? Infinity
    const parts = (v.takeaways ?? [])
      .flatMap((t): Part[] => {
        const at = secs(t.at)
        return at == null || at >= total - 3 ? [] : [{ say: clip(t.text, 160), moment: [at, Math.min(at + PART, total)] }]
      })
      .sort((a, b) => a.moment[0] - b.moment[0])
      // Takeaways can share a timestamp: a part that starts before the last kept one ends would replay it.
      .reduce<Part[]>((kept, p) => (kept.length && p.moment[0] < kept.at(-1)!.moment[1] ? kept : [...kept, p]), [])
      .slice(0, 4)
    if (!parts.length) continue
    channels.add(v.channel)
    chapters.push({ video: v, intro: `${v.channel}: ${clip(firstSentence(v.tldr || v.verdict || v.title), 180)}`, parts })
  }
  if (!chapters.length) return null
  return {
    intro: (written && script?.intro) || data.debrief?.headline || "Your week, channel by channel.",
    chapters,
    outro: written ? script?.outro : null,
    written,
  }
}

/** How long a line is on screen when nobody reads it out: about 2.7 words a second, between 3 and 12 s. */
const readMs = (text: string) => Math.min(12000, Math.max(3000, (text.split(/\s+/).length / 2.7) * 1000 + 500))

type Step =
  | { kind: "say"; ch: number; text: string; label: string; video: Card; cuts: number[] }
  | { kind: "part"; ch: number; part: number; video: Card; moment: [number, number] }

/**
 * Where a line's montage cuts start: 2-5 scenes (one per CUT_MS of the line), spread from `lo`, the last one running
 * straight into `end`. Scenes stay apart enough not to repeat each other.
 */
function cutsFor(ms: number, lo: number, end: number): number[] {
  const n = Math.min(5, Math.max(2, Math.round(ms / CUT_MS)))
  const source = (ms / n / 1000) * FAST
  const last = Math.max(0, end - source)
  const gap = Math.max(source * 1.6, (last - lo) / (n - 1))
  return Array.from({ length: n }, (_, i) => Math.max(0, last - gap * (n - 1 - i)))
}

/**
 * The running order. Under each line, a montage of quick cuts from the video; the last cut of a part's line runs into
 * the part, which then plays as it is. The week's intro and each takeaway cut across the whole video, like a trailer.
 * `lineMs` is how long a line takes: its narration once that's known.
 */
function stepsOf(show: ShowScript, lineMs: (text: string) => number): Step[] {
  const steps: Step[] = []
  show.chapters.forEach((c, ch) => {
    const total = secs(c.video.length) ?? c.parts.at(-1)!.moment[1] + 60
    if (ch === 0) steps.push({ kind: "say", ch, text: show.intro, label: "This week", video: c.video, cuts: cutsFor(lineMs(show.intro), total * 0.05, total * 0.45) })
    steps.push({ kind: "say", ch, text: c.intro, label: "The takeaway", video: c.video, cuts: cutsFor(lineMs(c.intro), total * 0.5, total * 0.95) })
    c.parts.forEach((p, i) => {
      const lo = i ? c.parts[i - 1]!.moment[1] : total * 0.03
      steps.push(
        { kind: "say", ch, text: p.say, label: `Part ${i + 1} of ${c.parts.length}`, video: c.video, cuts: cutsFor(lineMs(p.say), lo, p.moment[0]) },
        { kind: "part", ch, part: i, video: c.video, moment: p.moment }
      )
    })
  })
  if (show.outro) {
    const last = show.chapters.at(-1)!
    const total = secs(last.video.length) ?? last.parts.at(-1)!.moment[1] + 60
    steps.push({ kind: "say", ch: show.chapters.length - 1, text: show.outro, label: "That's the week", video: last.video, cuts: cutsFor(lineMs(show.outro), total * 0.1, total * 0.9) })
  }
  return steps
}

/** The debrief's length in seconds, for these chapters, before the narration is voiced. */
export const showSeconds = (s: ShowScript) =>
  stepsOf(s, readMs).reduce((a, x) => a + (x.kind === "say" ? readMs(x.text) : (x.moment[1] - x.moment[0]) * 1000), 0) / 1000

/** Closed captions: the line in cues of up to two short lines, the way YouTube shows them. */
function cues(text: string): string[] {
  const out: string[] = []
  let cur = ""
  for (const w of text.split(/\s+/).filter(Boolean)) {
    if (cur && (cur + " " + w).length > 84) {
      out.push(cur)
      cur = w
    } else cur = cur ? `${cur} ${w}` : w
  }
  if (cur) out.push(cur)
  return out
}

function clock(s: number): string {
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const ss = String(Math.floor(s % 60)).padStart(2, "0")
  return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`
}

/** Moves a media element's volume to `to` over `ms`, then calls `then`. */
function ramp(el: HTMLMediaElement, to: number, ms: number, then?: () => void) {
  const from = el.volume
  const t0 = performance.now()
  const tick = (now: number) => {
    const k = Math.min(1, (now - t0) / ms)
    el.volume = from + (to - from) * k
    if (k < 1) requestAnimationFrame(tick)
    else then?.()
  }
  requestAnimationFrame(tick)
}

// ---- What tx provides: each video's stream, and the narrator's voice ----

type Load<T> = { ok: true; value: T } | { ok: false; why: string }

/** Runs `work` for each key, two at a time, in order; results land in the map as they come. */
function useQueue<T>(keys: string[], work: (key: string) => Promise<T>, enabled: boolean) {
  const [done, setDone] = useState<ReadonlyMap<string, Load<T>>>(new Map())
  const asked = useRef(new Set<string>())
  const alive = useRef(true)
  const worker = useRef(work)
  useEffect(() => {
    worker.current = work
  })
  useEffect(() => {
    alive.current = true
    return () => {
      alive.current = false
    }
  }, [])
  useEffect(() => {
    if (!enabled) return
    const todo = keys.filter((k) => !asked.current.has(k))
    if (!todo.length) return
    for (const k of todo) asked.current.add(k)
    let next = 0
    const run = async () => {
      while (next < todo.length && alive.current) {
        const k = todo[next++]!
        let r: Load<T>
        try {
          r = { ok: true, value: await worker.current(k) }
        } catch (e) {
          r = { ok: false, why: e instanceof Error ? e.message : String(e) }
        }
        if (alive.current) setDone((m) => new Map(m).set(k, r))
      }
    }
    void run()
    void run()
  }, [keys, enabled])
  return done
}

const czech = /[ěščřžýůťďň]/i

/** A line's voice as a playable URL and its length, from tx's local narrator. */
async function narrate(text: string): Promise<{ url: string; ms: number }> {
  const r = await callTool("narrate", { text, lang: czech.test(text) ? "cs" : "en" })
  const sc = r.structuredContent
  if (!sc || typeof sc !== "object" || !("audio" in sc) || typeof sc.audio !== "string" || !("ms" in sc) || typeof sc.ms !== "number") {
    throw new Error("tx sent no narration.")
  }
  const bytes = Uint8Array.from(atob(sc.audio), (c) => c.charCodeAt(0))
  return { url: URL.createObjectURL(new Blob([bytes], { type: "audio/wav" })), ms: sc.ms }
}

async function stream(videoId: string): Promise<string> {
  const r = await callTool("video_stream", { video_id: videoId })
  const sc = r.structuredContent
  if (!sc || typeof sc !== "object" || !("url" in sc) || typeof sc.url !== "string") throw new Error("tx sent no stream for this video.")
  return sc.url
}

/** What one of the two players shows: a cut of a line's montage, or a part. */
interface Shot {
  key: string
  url: string
  from: number
  part: boolean
}

/**
 * "Watch your debrief": a narrated video over the week, one chapter per channel. Under each of the narrator's lines
 * (captions and tx's voice) runs a montage of quick cuts from the video; each key part then plays as it is, with its
 * sound faded in. Two players take turns: while one plays, the next cut is loaded and positioned in the other, so cuts
 * are instant and a new channel's video is ready before its chapter starts.
 */
export function Show({ show, onClose, canTools }: { show: ShowScript; onClose: () => void; canTools: boolean }) {
  const [at, setAt] = useState(0)
  const [paused, setPaused] = useState(false)
  const [sound, setSound] = useState(true)
  const [muteForced, setMuteForced] = useState(false)
  const [frac, setFrac] = useState(0)
  const [front, setFront] = useState(0)
  const [fade, setFade] = useState(false)
  // The two players, filled by their elements' ref callbacks.
  const players = useRef<(HTMLVideoElement | null)[]>([null, null])
  const frontRef = useRef(0)
  const prepared = useRef<{ key: string; el: number } | null>(null)
  const showing = useRef<Shot | null>(null)

  const videoIds = useMemo(() => show.chapters.map((c) => c.video.id), [show])
  const media = useQueue(videoIds, stream, canTools)
  const texts = useMemo(() => [show.intro, ...show.chapters.flatMap((c) => [c.intro, ...c.parts.map((p) => p.say)]), ...(show.outro ? [show.outro] : [])], [show])
  const voiced = useQueue(texts, narrate, canTools && sound)
  // tx couldn't voice a line: don't hold the show for the rest, captions carry it.
  const voiceDown = [...voiced.values()].some((v) => !v.ok)
  const lineMs = (text: string) => {
    const v = voiced.get(text)
    return v?.ok ? Math.max(1500, v.value.ms + 350) : readMs(text)
  }
  const steps = stepsOf(show, lineMs)
  const lengths = steps.map((s) => (s.kind === "say" ? lineMs(s.text) : (s.moment[1] - s.moment[0]) * 1000))
  const total = lengths.reduce((a, b) => a + b, 0)
  const chapterStart = show.chapters.map((_, ch) => steps.findIndex((s) => s.ch === ch))

  const done = at >= steps.length
  const step = done ? undefined : steps[at]
  const chapter = step ? show.chapters[step.ch] : undefined
  const elapsed = lengths.slice(0, at).reduce((a, b) => a + b, 0) + (step ? frac * lengths[at]! : 0)
  const source = step ? media.get(step.video.id) : undefined
  const needsVoice = step?.kind === "say" && sound && canTools && !voiceDown
  // A step starts once its video (or the news that there's none) and its voice are here.
  const ready = !!step && (!canTools || !!source) && (!needsVoice || voiced.has(step.text))
  const noVideo = !canTools || (source && !source.ok)
  const cut = step?.kind === "say" ? Math.min(step.cuts.length - 1, Math.floor(frac * step.cuts.length)) : 0

  /** The shot of step `i`, cut `c` (null when its video isn't streamable yet). */
  const shotOf = (i: number, c: number): Shot | null => {
    const s = steps[i]
    const m = s && media.get(s.video.id)
    if (!s || !m?.ok) return null
    return s.kind === "say" ? { key: `${i}:${c}`, url: m.value, from: s.cuts[c]!, part: false } : { key: `${i}:0`, url: m.value, from: s.moment[0], part: true }
  }
  const nextShot = (): Shot | null => (step?.kind === "say" && cut + 1 < step.cuts.length ? shotOf(at, cut + 1) : shotOf(at + 1, 0))

  /** Puts `shot` in `el`, positioned and paused, ready to start the moment it's shown. */
  const load = (el: HTMLVideoElement, shot: Shot) => {
    el.pause()
    const seek = () => {
      el.currentTime = shot.from
    }
    if (el.dataset.src !== shot.url) {
      el.dataset.src = shot.url
      el.src = shot.url
      el.addEventListener("loadedmetadata", seek, { once: true })
    } else seek()
  }

  // Each cut or part: show it in the player it was prepared in (else load it in the front one), start it, then get the
  // next one ready in the other player.
  useEffect(() => {
    const shot = step && ready ? shotOf(at, cut) : null
    if (!shot || showing.current?.key === shot.key) return
    const was = showing.current
    showing.current = shot
    const prev = frontRef.current
    const next = prepared.current?.key === shot.key ? prepared.current.el : prev
    const el = players.current[next]!
    if (next === prev) load(el, shot)
    prepared.current = null
    el.playbackRate = shot.part ? 1 : FAST
    el.muted = !shot.part || !sound || muteForced
    el.volume = shot.part ? 0 : 1
    if (!paused)
      void el.play().then(
        () => shot.part && !el.muted && ramp(el, 1, FADE_IN),
        () => {
          // Sound needs the app's leave: carry on muted, and say so.
          el.muted = true
          setMuteForced(true)
          void el.play().catch(() => {})
        }
      )
    // A new channel, or leaving a part, fades across; cuts within a montage are hard.
    setFade(!was || was.url !== shot.url || was.part !== shot.part)
    frontRef.current = next
    setFront(next)
    const old = players.current[1 - next]!
    const after = () => {
      if (next !== prev) old.pause()
      const upcoming = nextShot()
      if (upcoming) {
        load(old, upcoming)
        prepared.current = { key: upcoming.key, el: 1 - next }
      }
    }
    if (next !== prev && was?.part && !old.muted) ramp(old, 0, FADE_OUT, after)
    else after()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once per cut or part, when it's ready
  }, [at, cut, ready, media])

  useEffect(() => {
    const el = players.current[frontRef.current]
    if (!el || !showing.current?.part) return
    el.muted = !sound || muteForced
    if (!el.muted) el.volume = 1
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only the sound switch
  }, [sound])

  useEffect(() => {
    const el = players.current[frontRef.current]
    if (!el || !source?.ok) return
    if (paused || done || !ready) el.pause()
    else if (el.readyState >= HTMLMediaElement.HAVE_METADATA && showing.current) void el.play().catch(() => {})
  }, [paused, done, ready, source])

  // The narrator: tx's voice for the line; if tx can't voice it, the browser's own voice where there is one.
  useEffect(() => {
    if (step?.kind !== "say" || paused || !sound || !ready) return
    const v = voiced.get(step.text)
    if (v?.ok) {
      const a = new Audio(v.value.url)
      a.currentTime = (frac * lengths[at]!) / 1000
      void a.play().catch(() => {})
      return () => a.pause()
    }
    const synth = typeof speechSynthesis === "undefined" ? null : speechSynthesis
    const fallback = synth?.getVoices().find((x) => x.lang.startsWith(czech.test(step.text) ? "cs" : "en"))
    if (!synth || !fallback) return
    const u = new SpeechSynthesisUtterance(step.text)
    u.voice = fallback
    synth.cancel()
    synth.speak(u)
    return () => synth.cancel()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- per step, pause and sound; `frac` read once to resume
  }, [at, paused, sound, ready])

  // The clock: a line lasts its narration; a part until its video passes its end (or, if it stalls, a while more).
  useEffect(() => {
    if (!step || paused || !ready) return
    const length = lengths[at]!
    const began = performance.now() - frac * length
    let frame = 0
    const tick = (now: number) => {
      let f = (now - began) / length
      const el = players.current[frontRef.current]
      if (step.kind === "part" && el && source?.ok) {
        const [a, b] = step.moment
        if (el.currentTime >= a - 1) f = Math.max(f * 0.5, (el.currentTime - a) / (b - a))
        if (f < 1 && (now - began) / length > 1.6) f = 1
      }
      if (f >= 1) {
        setFrac(0)
        setAt((i) => i + 1)
        return
      }
      setFrac(f)
      frame = requestAnimationFrame(tick)
    }
    frame = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(frame)
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `frac` is read once to resume where a pause stopped
  }, [at, paused, ready])

  const jump = (i: number) => {
    setFrac(0)
    setPaused(false)
    setAt(Math.max(0, Math.min(i, steps.length)))
  }
  const chapterOf = (i: number) => (i >= steps.length ? show.chapters.length : steps[i]!.ch)
  const prevChapter = () => jump(chapterStart[Math.max(0, chapterOf(at) - (frac < 0.15 && steps[at - 1]?.ch !== steps[at]?.ch ? 1 : 0))] ?? 0)
  const nextChapter = () => jump(chapterStart[chapterOf(at) + 1] ?? steps.length)

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === " " || e.key === "k") setPaused((p) => !p)
      else if (e.key === "ArrowRight") jump(at + 1)
      else if (e.key === "ArrowLeft") jump(at - 1)
      else return
      e.preventDefault()
    }
    addEventListener("keydown", onKey)
    return () => removeEventListener("keydown", onKey)
  })

  const say = step?.kind === "say" ? step : null
  const lineCues = say ? cues(say.text) : []
  const cue = lineCues[Math.min(lineCues.length - 1, Math.floor(frac * lineCues.length))]
  const chapterMs = show.chapters.map((_, ch) => steps.reduce((a, s, i) => a + (s.ch === ch ? lengths[i]! : 0), 0))
  const width = "w-[min(100%,calc((var(--app-h)-190px)*16/9))]"

  return (
    <div className="mx-auto flex max-w-[1180px] flex-col gap-3 px-4 pt-3 pb-6 @min-[900px]:px-6">
      <div className="flex items-center justify-between gap-3">
        <Button variant="ghost" size="sm" className="-ml-2.5 font-semibold text-muted-foreground" onClick={onClose}>
          <ArrowLeftIcon data-icon="inline-start" />
          Your brief
        </Button>
        <ExpandButton />
      </div>

      <div className={cn("@container relative mx-auto aspect-video overflow-hidden rounded-xl bg-black text-white", width)}>
        {[0, 1].map((i) => (
          <video
            key={i}
            ref={(el) => {
              players.current[i] = el
            }}
            playsInline
            preload="auto"
            muted
            className={cn("absolute inset-0 size-full object-contain", front === i ? "opacity-100" : "opacity-0", fade && "transition-opacity duration-300", noVideo && "hidden")}
          />
        ))}
        {noVideo && step && <img key={step.video.id} src={step.video.thumbnail ?? ""} alt="" className="reel-pan absolute inset-0 size-full object-cover" />}

        {/* A light veil under a line, so the captions read; none over a part. */}
        <div aria-hidden className={cn("pointer-events-none absolute inset-0 transition-opacity duration-300", say || done ? "bg-black/30 opacity-100" : "opacity-0")} />

        {chapter && !done && (
          <div className="pointer-events-none absolute top-3 left-3 flex max-w-[80%] items-center gap-2 rounded-lg bg-black/55 py-1.5 pr-3 pl-1.5 text-[clamp(11px,1.5cqw,14px)] backdrop-blur-sm">
            <Thumb v={chapter.video} small className="w-12 shrink-0 rounded-[4px]" />
            <span className="min-w-0">
              <b className="block truncate font-semibold">{chapter.video.channel}</b>
              <span className="block truncate text-white/75">{chapter.video.title}</span>
            </span>
            {say && !noVideo && <span className="ml-1 shrink-0 rounded bg-white/20 px-1.5 font-mono text-[11px]">{FAST}×</span>}
          </div>
        )}

        {say && ready && (
          <div className="pointer-events-none absolute inset-x-0 bottom-[7%] grid justify-items-center gap-1.5 px-[8%]">
            <span className="rounded bg-black/70 px-2 py-0.5 text-[clamp(10px,1.3cqw,13px)] font-semibold tracking-[.06em] text-white/80 uppercase">{say.label}</span>
            <p key={`${at}-${cue}`} className="reel-word m-0 max-w-[46ch] rounded-md bg-black/78 px-3 py-1.5 text-center text-[clamp(15px,2.6cqw,28px)] leading-snug font-medium text-pretty">
              {cue}
            </p>
          </div>
        )}

        {step?.kind === "part" && ready && (
          <div className="pointer-events-none absolute bottom-4 left-4 flex items-center gap-2 rounded-md bg-black/55 px-2 py-1 text-[12.5px] font-semibold backdrop-blur-sm">
            {noVideo ? (
              <ExtLink url={`https://www.youtube.com/watch?v=${step.video.id}&t=${step.moment[0]}s`} className="pointer-events-auto underline">
                Play this part on YouTube
              </ExtLink>
            ) : (
              <>
                <span className="font-mono">{clock(step.moment[0])}</span>
                {muteForced ? "Muted: this app doesn't let it play sound" : `Part ${step.part + 1} of ${chapter!.parts.length}, as it's said`}
              </>
            )}
          </div>
        )}

        {step && !ready && (
          <div className="absolute inset-0 z-20 grid place-content-center justify-items-center gap-3 bg-black/60 text-center text-[14px] text-white/85">
            <Spinner className="size-6" />
            {!source ? "Getting the video…" : voiced.size === 0 ? "Voicing the narrator… The first time, tx downloads its voice." : "Voicing the narrator…"}
          </div>
        )}

        {done ? (
          <div className="absolute inset-0 z-20 grid place-content-center justify-items-center gap-4 px-8 text-center">
            <p className="m-0 font-slate text-[clamp(22px,4cqw,40px)] leading-[1.05]">That's your debrief.</p>
            <div className="flex gap-2">
              <Button variant="secondary" onClick={() => jump(0)}>
                <RotateCcwIcon data-icon="inline-start" />
                Watch again
              </Button>
              <Button variant="ghost" className="text-white/80 hover:bg-white/10 hover:text-white" onClick={onClose}>
                Back to your brief
              </Button>
            </div>
          </div>
        ) : (
          <button type="button" aria-label={paused ? "Play" : "Pause"} className="absolute inset-0 z-10 grid cursor-pointer place-items-center" onClick={() => setPaused((p) => !p)}>
            {paused && (
              <span className="grid size-16 place-items-center rounded-full bg-black/55">
                <PlayIcon className="size-7" />
              </span>
            )}
          </button>
        )}
      </div>

      {/* The progress bar, in chapters (one per channel), like a YouTube video's. */}
      <div className={cn("mx-auto", width)}>
        <div className="flex h-1.5 gap-[3px]">
          {show.chapters.map((c, ch) => {
            const before = chapterMs.slice(0, ch).reduce((a, b) => a + b, 0)
            const filled = Math.max(0, Math.min(1, (elapsed - before) / chapterMs[ch]!))
            return (
              <button
                key={ch}
                type="button"
                title={`${c.video.channel}: ${c.video.title}`}
                onClick={() => jump(chapterStart[ch]!)}
                style={{ flexGrow: chapterMs[ch] }}
                className="relative basis-0 cursor-pointer overflow-hidden rounded-full bg-foreground/15 hover:bg-foreground/25"
              >
                <span className="absolute inset-y-0 left-0 bg-destructive" style={{ width: `${(done ? 1 : filled) * 100}%` }} />
              </button>
            )
          })}
        </div>
        <div className="mt-2 flex items-center gap-1.5">
          <Button variant="ghost" size="icon-sm" aria-label="Previous chapter" onClick={prevChapter}>
            <SkipBackIcon />
          </Button>
          <Button variant="ghost" size="icon-sm" aria-label={paused ? "Play" : "Pause"} disabled={done} onClick={() => setPaused((p) => !p)}>
            {paused || done ? <PlayIcon /> : <PauseIcon />}
          </Button>
          <Button variant="ghost" size="icon-sm" aria-label="Next chapter" disabled={done} onClick={nextChapter}>
            <SkipForwardIcon />
          </Button>
          <Button variant="ghost" size="icon-sm" aria-label={sound ? "Sound off" : "Sound on"} title={voiceDown ? "tx couldn't voice the narrator: the captions carry it" : undefined} onClick={() => setSound((s) => !s)}>
            {sound ? <Volume2Icon /> : <VolumeXIcon />}
          </Button>
          <span className="ml-1 font-mono text-[12px] text-muted-foreground tabular-nums">
            {clock(elapsed / 1000)} / {clock(total / 1000)}
          </span>
          {chapter && !done && <span className="ml-2 truncate text-[13px] text-muted-foreground">· {chapter.video.channel}</span>}
          {voiceDown && <span className="ml-auto text-[12px] text-faint">Narrator voice unavailable: captions only</span>}
        </div>
      </div>

      <ol className={cn("mx-auto m-0 grid list-none gap-1 p-0", width)}>
        {show.chapters.map((c, ch) => (
          <li key={ch}>
            <button
              type="button"
              onClick={() => jump(chapterStart[ch]!)}
              className={cn("grid w-full grid-cols-[22px_72px_1fr] items-center gap-2.5 rounded-lg px-1.5 py-1.5 text-left hover:bg-muted", step?.ch === ch && !done && "bg-muted")}
            >
              <span className="text-center font-mono text-[12px] text-faint">{ch + 1}</span>
              <Thumb v={c.video} small className="rounded-[4px]" />
              <span className="min-w-0">
                <span className="block truncate text-[13px] font-semibold">{c.video.channel}</span>
                <span className="block truncate text-[12px] text-muted-foreground">
                  {c.video.title} · {plural(c.parts.length, "key part")}
                </span>
              </span>
            </button>
          </li>
        ))}
      </ol>
      {!show.written && (
        <p className="mx-auto m-0 max-w-[70ch] text-center text-[12px] text-faint">
          Made from the analyses' takeaways. After the next screening, your AI writes the narration and picks the parts.
        </p>
      )}
    </div>
  )
}
