import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import { ArrowLeftIcon, PauseIcon, PlayIcon, SkipBackIcon, SkipForwardIcon, Volume2Icon, VolumeXIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { ExpandButton, ExtLink, Thumb, kindColor, kindOf } from "@/components/bits"
import { span, worth } from "@/components/landing"
import { cn } from "@/lib/utils"
import { plural, type Locale } from "@/lib/format"
import type { BriefData, Card } from "@/types"

/** One stretch of a video, and the narrator's line before it. */
export interface Clip {
  video: Card
  start: number
  end: number
  say: string
}

export interface ShowScript {
  intro: string
  clips: Clip[]
  outro?: string | null
  /** Written by the user's AI as a show; false when made up here from the picks' verdicts. */
  written: boolean
}

/** A fallback clip of a Watch: the start, at most this long. */
const WATCH_CLIP = 480

/**
 * The narrated show over the brief: your AI's script when it wrote one, else the picks in order, each introduced by its
 * verdict and cut to the stretch worth watching. YouTube only: podcast audio can't play inside the app's frame.
 */
export function showOf(data: BriefData): ShowScript | null {
  const byId = new Map(data.videos.map((v) => [v.id, v]))
  const script = data.debrief?.script
  if (script) {
    const clips = script.clips.flatMap((c) => {
      const video = byId.get(c.video_id)
      return video && !video.podcast ? [{ video, start: c.start, end: c.end, say: c.say }] : []
    })
    if (clips.length) return { intro: script.intro, clips, outro: script.outro, written: true }
  }
  const clips = data.videos.flatMap((v): Clip[] => {
    const k = kindOf(v)
    const w = worth(v)
    if (v.podcast || !w || (k !== "watch" && k !== "skim")) return []
    return [{ video: v, start: w[0], end: k === "watch" ? Math.min(w[1], WATCH_CLIP) : w[1], say: `${v.channel}. ${v.verdict ?? v.title}` }]
  })
  if (!clips.length) return null
  return {
    intro: data.debrief?.headline ?? `Your ${plural(clips.length, "pick")} from the brief, the parts worth watching.`,
    clips: clips.slice(0, 8),
    written: false,
  }
}

export const showLength = (s: ShowScript) => s.clips.reduce((a, c) => a + c.end - c.start, 0)

function clock(s: number): string {
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const ss = String(Math.floor(s % 60)).padStart(2, "0")
  return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`
}

// ---- YouTube IFrame API: the one script the frame loads from outside (declared in the view's CSP) ----

interface YTPlayer {
  loadVideoById(o: { videoId: string; startSeconds: number; endSeconds: number }): void
  playVideo(): void
  pauseVideo(): void
  getCurrentTime(): number
  getPlayerState(): number
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
const ENDED = 0
const PLAYING = 1
const BUFFERING = 3

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

// ---- The narrator: the browser's own voice where there is one, else the line stays on screen for its reading time ----

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

/** How long a line stays up when nobody reads it out: about 3 words a second, at least 3 s. */
const readingMs = (text: string) => Math.max(3000, (text.split(/\s+/).length / 3) * 1000 + 800)

type Step = { kind: "say"; text: string; clip?: number } | { kind: "clip"; clip: number }

/**
 * "Watch your debrief": the narrator's intro, then each clip set up by one line and played from its start to its end,
 * then the outro. One YouTube player stays mounted for the whole show, so clips follow each other without a reload.
 */
export function Show({ show, loc, onClose }: { show: ShowScript; loc: Locale; onClose: () => void }) {
  const steps = useMemo<Step[]>(
    () => [
      { kind: "say", text: show.intro },
      ...show.clips.flatMap((c, i): Step[] => [
        { kind: "say", text: c.say, clip: i },
        { kind: "clip", clip: i },
      ]),
      ...(show.outro ? [{ kind: "say" as const, text: show.outro }] : []),
    ],
    [show]
  )
  const [at, setAt] = useState(0)
  const [paused, setPaused] = useState(false)
  const [voiceOn, setVoiceOn] = useState(true)
  const [ready, setReady] = useState(false)
  const [blocked, setBlocked] = useState<string | null>(null)
  // Readings belong to the step they were taken in, so a new clip starts clean without resetting state.
  const [stalledAt, setStalledAt] = useState<number | null>(null)
  const [time, setTime] = useState({ at: -1, t: 0 })
  const holder = useRef<HTMLDivElement>(null)
  const player = useRef<YTPlayer | null>(null)
  // The player's events are bound once; they reach the current step through this.
  const advance = useRef(() => {})
  useLayoutEffect(() => {
    advance.current = () => setAt((i) => Math.min(i + 1, steps.length))
  }, [steps.length])
  const voices = useVoices()
  const voice = useMemo(() => pickVoice(voices, (loc.locale ?? navigator.language).slice(0, 2).toLowerCase()), [voices, loc.locale])

  const step = steps[at] as Step | undefined
  const clipIndex = step ? step.clip : undefined
  const clip = clipIndex != null ? show.clips[clipIndex] : undefined
  const done = at >= steps.length

  useEffect(() => {
    let gone = false
    loadYouTube().then(
      (YT) => {
        if (gone || !holder.current) return
        player.current = new YT.Player(holder.current, {
          width: "100%",
          height: "100%",
          host: "https://www.youtube-nocookie.com",
          playerVars: { playsinline: 1, rel: 0, modestbranding: 1, iv_load_policy: 3 },
          events: {
            onReady: () => setReady(true),
            onStateChange: (e) => {
              if (e.data === ENDED) advance.current()
              if (e.data === PLAYING || e.data === BUFFERING) setStalledAt(null)
            },
            onError: () => advance.current(),
          },
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

  // A clip: load its stretch (it plays on its own). If the app won't let it start, say so: the player's own button works.
  useEffect(() => {
    if (step?.kind !== "clip" || !ready || !clip) return
    player.current?.loadVideoById({ videoId: clip.video.id, startSeconds: clip.start, endSeconds: clip.end })
    const t = setTimeout(() => {
      const s = player.current?.getPlayerState()
      if (s !== PLAYING && s !== BUFFERING) setStalledAt(at)
    }, 2500)
    const tick = setInterval(() => setTime({ at, t: player.current?.getCurrentTime() ?? 0 }), 500)
    return () => {
      clearTimeout(t)
      clearInterval(tick)
    }
  }, [step, ready, clip, at])

  useEffect(() => {
    if (step?.kind !== "clip" || !ready) return
    if (paused) player.current?.pauseVideo()
    else player.current?.playVideo()
  }, [paused, step, ready])

  // A line: spoken, then on; or shown for its reading time. Pausing stops it; playing says it again.
  useEffect(() => {
    if (step?.kind !== "say" || paused) return
    player.current?.pauseVideo()
    if (voiceOn && voice) {
      const u = new SpeechSynthesisUtterance(step.text)
      u.voice = voice
      u.lang = voice.lang
      let over = false
      u.onend = u.onerror = () => {
        if (!over) advance.current()
        over = true
      }
      speechSynthesis.cancel()
      speechSynthesis.speak(u)
      return () => {
        over = true
        speechSynthesis.cancel()
      }
    }
    const t = setTimeout(() => advance.current(), readingMs(step.text))
    return () => clearTimeout(t)
  }, [step, paused, voiceOn, voice])

  const total = showLength(show)
  const jump = (i: number) => {
    setPaused(false)
    setAt(Math.max(0, Math.min(i, steps.length - 1)))
  }
  const clipStep = (i: number) => steps.findIndex((s) => s.kind === "say" && s.clip === i)

  return (
    <div className="mx-auto grid max-w-[980px] gap-3 px-4 pt-3 pb-6 @min-[900px]:px-6">
      <div className="flex items-center justify-between gap-3">
        <Button variant="ghost" size="sm" className="-ml-2.5 font-semibold text-muted-foreground" onClick={onClose}>
          <ArrowLeftIcon data-icon="inline-start" />
          Your brief
        </Button>
        <span className="truncate text-[12.5px] text-faint tabular-nums">
          Your debrief · {span(total)} · {plural(show.clips.length, "clip")}
        </span>
        <ExpandButton />
      </div>

      {/* The stage: the player stays mounted under the narrator's card, so the next clip starts without a reload. */}
      <div className="relative mx-auto aspect-video w-full max-w-[calc((var(--app-h)-150px)*16/9)] overflow-hidden rounded-xl bg-black">
        <div className="absolute inset-0">
          <div ref={holder} className="size-full" />
        </div>
        {(step?.kind !== "clip" || blocked) && (
          <div className="absolute inset-0 grid content-center gap-3 bg-[#111113] px-[8%] text-white">
            {done ? (
              <>
                <p className="m-0 font-slate text-[clamp(22px,4.2cqw,40px)] leading-[1.05]">That's your debrief.</p>
                <div className="flex gap-2">
                  <Button variant="secondary" onClick={() => jump(0)}>
                    Watch again
                  </Button>
                  <Button variant="ghost" className="text-white/80 hover:text-white" onClick={onClose}>
                    Back to your brief
                  </Button>
                </div>
              </>
            ) : step?.kind === "say" ? (
              <>
                <p className="m-0 text-[11px] font-bold tracking-[.1em] text-white/50 uppercase">
                  {clip ? `Clip ${clipIndex! + 1} of ${show.clips.length} · ${clip.video.channel} · ${clock(clip.start)}–${clock(clip.end)}` : at === 0 ? "This week" : "To close"}
                </p>
                <p className="m-0 max-w-[30ch] font-slate text-[clamp(20px,3.6cqw,36px)] leading-[1.08] text-balance">{step.text}</p>
                {clip && <p className="m-0 truncate text-[13px] text-white/60">{clip.video.title}</p>}
                {!(voiceOn && voice) && !paused && (
                  <div key={at} className="h-0.5 overflow-hidden rounded-full bg-white/10">
                    <div className="reel-lit h-full bg-white/60" style={{ animationDuration: `${readingMs(step.text)}ms`, animationTimingFunction: "linear" }} />
                  </div>
                )}
              </>
            ) : (
              clip && (
                <>
                  <p className="m-0 text-[14px] text-white/80">{blocked} Play this stretch on YouTube instead:</p>
                  <ExtLink url={`https://www.youtube.com/watch?v=${clip.video.id}&t=${clip.start}s`} className="font-semibold underline">
                    {clip.video.title} · {clock(clip.start)}–{clock(clip.end)}
                  </ExtLink>
                </>
              )
            )}
          </div>
        )}
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Button variant="outline" size="icon-sm" aria-label="Previous" onClick={() => jump(clipIndex != null ? clipStep(Math.max(0, clipIndex - 1)) : at - 1)}>
          <SkipBackIcon />
        </Button>
        <Button variant="outline" size="icon-sm" aria-label={paused ? "Play" : "Pause"} disabled={done} onClick={() => setPaused((p) => !p)}>
          {paused ? <PlayIcon /> : <PauseIcon />}
        </Button>
        <Button variant="outline" size="icon-sm" aria-label="Next" disabled={done} onClick={() => jump(at + 1)}>
          <SkipForwardIcon />
        </Button>
        <span className="ml-1 text-[12.5px] text-muted-foreground tabular-nums">
          {done
            ? "Finished"
            : step?.kind === "clip" && clip
              ? `Clip ${clipIndex! + 1} of ${show.clips.length} · ${clock(Math.max(0, (time.at === at ? time.t : clip.start) - clip.start))} of ${clock(clip.end - clip.start)}`
              : "Narrator"}
        </span>
        {stalledAt === at && step?.kind === "clip" && <span className="text-[12.5px] text-skim">The app won't start it on its own: press play on the video.</span>}
        <Button
          variant="ghost"
          size="sm"
          className="ml-auto text-muted-foreground"
          disabled={!voice}
          title={voice ? `Narrator: ${voice.name}` : "No voice on this computer: the narrator's lines are shown instead"}
          onClick={() => setVoiceOn((v) => !v)}
        >
          {voiceOn && voice ? <Volume2Icon data-icon="inline-start" /> : <VolumeXIcon data-icon="inline-start" />}
          {voice ? (voiceOn ? "Narrator on" : "Narrator off") : "No voice: text only"}
        </Button>
      </div>

      {/* The running order: each clip as long as it plays; the current one lit. */}
      <ol className="m-0 grid list-none gap-1 p-0">
        {show.clips.map((c, i) => {
          const current = clipIndex === i
          return (
            <li key={i}>
              <button
                type="button"
                onClick={() => jump(clipStep(i))}
                className={cn("grid w-full grid-cols-[22px_72px_1fr] items-center gap-2.5 rounded-lg px-1.5 py-1.5 text-left hover:bg-muted", current && "bg-muted", kindColor[kindOf(c.video)])}
              >
                <span className={cn("text-center font-mono text-[12px] text-faint", current && "font-bold text-(--k)")}>{i + 1}</span>
                <Thumb v={c.video} small className="rounded-[4px]" />
                <span className="min-w-0">
                  <span className="block truncate text-[13px] font-medium">{c.video.title}</span>
                  <span className="block truncate text-[12px] text-faint">
                    {c.video.channel} · {clock(c.start)}–{clock(c.end)} · {span(c.end - c.start)}
                  </span>
                </span>
              </button>
            </li>
          )
        })}
      </ol>
      {!show.written && <p className="m-0 text-[12px] text-faint">Made from the picks' verdicts. After the next screening, your AI writes the narration itself.</p>}
    </div>
  )
}
