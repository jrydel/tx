import { useEffect, useMemo, useRef, useState } from "react"
import { ArrowLeftIcon, PauseIcon, PlayIcon, RotateCcwIcon, SkipBackIcon, SkipForwardIcon, Volume2Icon, VolumeXIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { ExpandButton, ExtLink, Thumb, kindOf } from "@/components/bits"
import { cn } from "@/lib/utils"
import { plural, secs, type Locale } from "@/lib/format"
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
/** The fastest YouTube's player goes: the background runs at this under the narrator. */
const FAST = 2

const firstSentence = (t: string) => t.split(/(?<=[.!?])\s/)[0]!.trim()
const clip = (t: string, n: number) => (t.length > n ? `${t.slice(0, n - 1).trimEnd()}…` : t)

/**
 * Every channel's chapter the brief can offer: your AI's script first, in its order, then a chapter made from the
 * analysis for each other channel with a pick (its best video: the TL;DR as the takeaway, the timestamped takeaways as
 * the key parts). YouTube only: podcast audio can't play inside the app's frame.
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

/** How long a line is on screen: about 2.7 words a second, between 3 and 12 s. */
const readMs = (text: string) => Math.min(12000, Math.max(3000, (text.split(/\s+/).length / 2.7) * 1000 + 500))

type Step =
  | { kind: "say"; ch: number; text: string; label: string; video: Card; from: number }
  | { kind: "part"; ch: number; part: number; video: Card; moment: [number, number] }

/**
 * The running order. Each line runs over its video at double speed from far enough back to arrive at the next part as
 * the line ends, so the part just carries on at normal speed. A chapter's takeaway leads into its first line the same way.
 */
function stepsOf(show: ShowScript): Step[] {
  const steps: Step[] = []
  show.chapters.forEach((c, ch) => {
    const lines: Step[] = []
    let next = c.parts[0]!.moment[0]
    const lead = (text: string, upTo: number) => Math.max(0, upTo - (readMs(text) / 1000) * FAST)
    const partSteps = c.parts.flatMap((p, i): Step[] => {
      const from = lead(p.say, p.moment[0])
      if (i === 0) next = from
      return [
        { kind: "say", ch, text: p.say, label: `Part ${i + 1} of ${c.parts.length}`, video: c.video, from },
        { kind: "part", ch, part: i, video: c.video, moment: p.moment },
      ]
    })
    if (ch === 0) {
      const introFrom = lead(c.intro, next)
      lines.push({ kind: "say", ch, text: show.intro, label: "This week", video: c.video, from: lead(show.intro, introFrom) })
    }
    lines.push({ kind: "say", ch, text: c.intro, label: "The takeaway", video: c.video, from: lead(c.intro, next) })
    steps.push(...lines, ...partSteps)
  })
  if (show.outro) {
    const last = show.chapters.at(-1)!
    steps.push({ kind: "say", ch: show.chapters.length - 1, text: show.outro, label: "That's the week", video: last.video, from: last.parts.at(-1)!.moment[1] })
  }
  return steps
}

const stepMs = (s: Step) => (s.kind === "say" ? readMs(s.text) : (s.moment[1] - s.moment[0]) * 1000)

/** The debrief's length in seconds, for these chapters. */
export const showSeconds = (s: ShowScript) => stepsOf(s).reduce((a, x) => a + stepMs(x), 0) / 1000

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

// ---- YouTube IFrame API: the one script the frame loads from outside (declared in the view's CSP) ----

interface YTPlayer {
  loadVideoById(o: { videoId: string; startSeconds: number }): void
  seekTo(s: number, allowSeekAhead: boolean): void
  playVideo(): void
  pauseVideo(): void
  mute(): void
  unMute(): void
  setPlaybackRate(r: number): void
  getCurrentTime(): number
  destroy(): void
}
interface YTApi {
  Player: new (
    el: HTMLElement,
    o: {
      width: string
      height: string
      host: string
      playerVars: Record<string, number>
      events: { onReady: () => void; onStateChange: (e: { data: number }) => void; onError: () => void }
    }
  ) => YTPlayer
}
const PLAYING = 1
const PAUSED = 2

let youtube: Promise<YTApi> | null = null
function loadYouTube(): Promise<YTApi> {
  if (youtube) return youtube
  const { promise, resolve, reject } = Promise.withResolvers<YTApi>()
  const w = window as unknown as { YT?: YTApi; onYouTubeIframeAPIReady?: () => void }
  w.onYouTubeIframeAPIReady = () => resolve(w.YT!)
  const s = document.createElement("script")
  s.src = "https://www.youtube.com/iframe_api"
  s.onerror = () => {
    youtube = null
    reject(new Error("The app blocked YouTube's player."))
  }
  document.head.append(s)
  youtube = promise
  return promise
}

// ---- The narrator: the browser's own voice where there is one; the captions carry the line either way ----

/** A natural voice for `lang` if the system has one; eSpeak-style robot voices only when nothing better exists. */
function pickVoice(voices: SpeechSynthesisVoice[], lang: string): SpeechSynthesisVoice | null {
  const fits = voices.filter((v) => v.lang.toLowerCase().startsWith(lang))
  const pool = fits.length ? fits : voices.filter((v) => v.lang.toLowerCase().startsWith("en"))
  return pool.find((v) => /natural|neural|premium|enhanced|google/i.test(v.name)) ?? pool.find((v) => !/espeak/i.test(v.name)) ?? pool[0] ?? null
}

function useVoices(): SpeechSynthesisVoice[] {
  const synth = typeof speechSynthesis === "undefined" ? null : speechSynthesis
  const [voices, setVoices] = useState<SpeechSynthesisVoice[]>(() => synth?.getVoices() ?? [])
  useEffect(() => {
    if (!synth) return
    const on = () => setVoices(synth.getVoices())
    synth.addEventListener("voiceschanged", on)
    return () => synth.removeEventListener("voiceschanged", on)
  }, [synth])
  return voices
}

/**
 * "Watch your debrief": a narrated video over the week, one chapter per channel. The narrator's lines run as closed
 * captions over the video at double speed; each key part then plays as it is, with sound. Chapters on the progress bar.
 */
export function Show({ show, loc, onClose }: { show: ShowScript; loc: Locale; onClose: () => void }) {
  const steps = useMemo(() => stepsOf(show), [show])
  const lengths = useMemo(() => steps.map(stepMs), [steps])
  const total = lengths.reduce((a, b) => a + b, 0)
  const chapterStart = useMemo(() => show.chapters.map((_, ch) => steps.findIndex((s) => s.ch === ch)), [show, steps])
  const [at, setAt] = useState(0)
  const [paused, setPaused] = useState(false)
  const [sound, setSound] = useState(true)
  const [ready, setReady] = useState(false)
  const [blocked, setBlocked] = useState<string | null>(null)
  const [muteForced, setMuteForced] = useState(false)
  const [frac, setFrac] = useState(0)
  const holder = useRef<HTMLDivElement>(null)
  const player = useRef<YTPlayer | null>(null)
  const loaded = useRef<string | null>(null)
  // What the player should be doing; re-applied whenever it (re)starts playing, since a load resets rate and volume.
  const want = useRef({ rate: FAST, muted: true, part: false })
  const onEvent = useRef<(s: number) => void>(() => {})
  const voices = useVoices()
  const voice = useMemo(() => pickVoice(voices, (loc.locale ?? navigator.language).slice(0, 2).toLowerCase()), [voices, loc.locale])

  const done = at >= steps.length
  const step = done ? undefined : steps[at]
  const chapter = step ? show.chapters[step.ch] : undefined
  const elapsed = lengths.slice(0, at).reduce((a, b) => a + b, 0) + (step ? frac * lengths[at]! : 0)

  const jump = (i: number) => {
    setFrac(0)
    setPaused(false)
    setAt(Math.max(0, Math.min(i, steps.length)))
  }

  const apply = () => {
    const p = player.current
    if (!p) return
    p.setPlaybackRate(want.current.rate)
    if (want.current.muted) p.mute()
    else p.unMute()
  }

  useEffect(() => {
    let gone = false
    loadYouTube().then(
      (YT) => {
        if (gone || !holder.current) return
        player.current = new YT.Player(holder.current, {
          width: "100%",
          height: "100%",
          host: "https://www.youtube-nocookie.com",
          playerVars: { controls: 0, disablekb: 1, fs: 0, iv_load_policy: 3, modestbranding: 1, playsinline: 1, rel: 0 },
          events: { onReady: () => setReady(true), onStateChange: (e) => onEvent.current(e.data), onError: () => setBlocked("This video won't play here.") },
        })
      },
      (e: Error) => !gone && setBlocked(e.message)
    )
    return () => {
      gone = true
      player.current?.destroy()
      player.current = null
    }
  }, [])

  useEffect(() => {
    onEvent.current = (s) => {
      if (s === PLAYING) apply()
      // Unmuting without the app's leave to play sound pauses the video: carry on muted, and say so.
      if (s === PAUSED && !paused && want.current.part && !want.current.muted) {
        want.current.muted = true
        setMuteForced(true)
        player.current?.mute()
        player.current?.playVideo()
      }
    }
  })

  // A line: its video, from where it leads into the next part, fast and muted. A part: as it is, from its start.
  useEffect(() => {
    const p = player.current
    if (!ready || !p || !step) return
    const target = step.kind === "say" ? step.from : step.moment[0]
    want.current = step.kind === "say" ? { rate: FAST, muted: true, part: false } : { rate: 1, muted: !sound || muteForced, part: true }
    if (loaded.current !== step.video.id) {
      loaded.current = step.video.id
      p.loadVideoById({ videoId: step.video.id, startSeconds: target })
    } else {
      if (Math.abs(p.getCurrentTime() - target) > 1.5) p.seekTo(target, true)
      apply()
    }
    if (!paused) p.playVideo()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- re-run on a new step, not on every sound or pause change
  }, [at, ready])

  useEffect(() => {
    if (step?.kind !== "part") return
    want.current.muted = !sound || muteForced
    apply()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only the sound switch
  }, [sound])

  useEffect(() => {
    const p = player.current
    if (!p || !ready) return
    if (paused || done) p.pauseVideo()
    else p.playVideo()
  }, [paused, ready, done])

  // The narrator reads the line, when there's a voice and sound is on.
  useEffect(() => {
    if (step?.kind !== "say" || paused || !sound || !voice) return
    const u = new SpeechSynthesisUtterance(step.text)
    u.voice = voice
    u.lang = voice.lang
    u.rate = 1.05
    speechSynthesis.cancel()
    speechSynthesis.speak(u)
    return () => speechSynthesis.cancel()
  }, [step, paused, sound, voice])

  // The clock: a line lasts its reading time; a part until the player passes its end (or, if it stalls, a while more).
  useEffect(() => {
    if (!step || paused) return
    const length = lengths[at]!
    const began = performance.now() - frac * length
    let frame = 0
    const tick = (now: number) => {
      let f = (now - began) / length
      if (step.kind === "part" && player.current && !blocked) {
        const t = player.current.getCurrentTime()
        const [a, b] = step.moment
        if (t >= a - 1) f = Math.max(f * 0.5, (t - a) / (b - a))
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
  }, [at, paused, blocked])

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
  // Chapters on the progress bar, as wide as they run.
  const chapterMs = show.chapters.map((_, ch) => steps.reduce((a, s, i) => a + (s.ch === ch ? lengths[i]! : 0), 0))

  return (
    <div className="mx-auto flex max-w-[1180px] flex-col gap-3 px-4 pt-3 pb-6 @min-[900px]:px-6">
      <div className="flex items-center justify-between gap-3">
        <Button variant="ghost" size="sm" className="-ml-2.5 font-semibold text-muted-foreground" onClick={onClose}>
          <ArrowLeftIcon data-icon="inline-start" />
          Your brief
        </Button>
        <ExpandButton />
      </div>

      <div className="@container relative mx-auto aspect-video w-[min(100%,calc((var(--app-h)-190px)*16/9))] overflow-hidden rounded-xl bg-black text-white">
        <div className="pointer-events-none absolute inset-0 [&>iframe]:size-full">
          <div ref={holder} />
        </div>
        {blocked && step && <img key={step.video.id} src={step.video.thumbnail ?? ""} alt="" className="reel-pan absolute inset-0 size-full object-cover" />}

        {/* A light veil under a line, so the captions read; none over a part. */}
        <div aria-hidden className={cn("pointer-events-none absolute inset-0 transition-opacity duration-300", say || done ? "bg-black/35 opacity-100" : "opacity-0")} />

        {chapter && !done && (
          <div className="pointer-events-none absolute top-3 left-3 flex max-w-[80%] items-center gap-2 rounded-lg bg-black/55 py-1.5 pr-3 pl-1.5 text-[clamp(11px,1.5cqw,14px)] backdrop-blur-sm">
            <Thumb v={chapter.video} small className="w-12 shrink-0 rounded-[4px]" />
            <span className="min-w-0">
              <b className="block truncate font-semibold">{chapter.video.channel}</b>
              <span className="block truncate text-white/75">{chapter.video.title}</span>
            </span>
            {say && !blocked && <span className="ml-1 shrink-0 rounded bg-white/20 px-1.5 font-mono text-[11px]">{FAST}×</span>}
          </div>
        )}

        {say && (
          <div className="pointer-events-none absolute inset-x-0 bottom-[7%] grid justify-items-center gap-1.5 px-[8%]">
            <span className="rounded bg-black/70 px-2 py-0.5 text-[clamp(10px,1.3cqw,13px)] font-semibold tracking-[.06em] text-white/80 uppercase">{say.label}</span>
            <p key={`${at}-${cue}`} className="reel-word m-0 max-w-[46ch] rounded-md bg-black/78 px-3 py-1.5 text-center text-[clamp(15px,2.6cqw,28px)] leading-snug font-medium text-pretty">
              {cue}
            </p>
          </div>
        )}

        {step?.kind === "part" && (
          <div className="pointer-events-none absolute bottom-4 left-4 flex items-center gap-2 rounded-md bg-black/55 px-2 py-1 text-[12.5px] font-semibold backdrop-blur-sm">
            {blocked ? (
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
      <div className="mx-auto w-[min(100%,calc((var(--app-h)-190px)*16/9))]">
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
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={sound ? "Sound off" : "Sound on"}
            title={voice ? `Narrator: ${voice.name}` : "No voice on this computer: the captions carry the narration"}
            onClick={() => setSound((s) => !s)}
          >
            {sound ? <Volume2Icon /> : <VolumeXIcon />}
          </Button>
          <span className="ml-1 font-mono text-[12px] text-muted-foreground tabular-nums">
            {clock(elapsed / 1000)} / {clock(total / 1000)}
          </span>
          {chapter && !done && <span className="ml-2 truncate text-[13px] text-muted-foreground">· {chapter.video.channel}</span>}
        </div>
      </div>

      <ol className="mx-auto m-0 grid w-[min(100%,calc((var(--app-h)-190px)*16/9))] list-none gap-1 p-0">
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
