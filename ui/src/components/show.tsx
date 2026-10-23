import { useEffect, useMemo, useRef, useState } from "react"
import { ArrowLeftIcon, PlayIcon, RotateCcwIcon, Volume2Icon, VolumeXIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { ExpandButton, ExtLink, kindOf } from "@/components/bits"
import { worth } from "@/components/landing"
import { cn } from "@/lib/utils"
import { plural, secs, type Locale } from "@/lib/format"
import type { BriefData, Card } from "@/types"

/** One video in the reel: the stretch that runs fast behind the narrator's line, and the moment played as it is. */
export interface Clip {
  video: Card
  start: number
  end: number
  say: string
  moment: [number, number]
}

export interface ShowScript {
  intro: string
  clips: Clip[]
  outro?: string | null
  /** Written by the user's AI; false when made up here from the picks' verdicts. */
  written: boolean
}

/** A moment, when the script names none: this long from the first takeaway inside the stretch. */
const MOMENT = 12
/** The fastest YouTube's player goes. */
const FAST = 2
const MAX_CLIPS = 6

/** The verdict's first sentence, short enough for a caption. */
function shortLine(v: Card): string {
  const first = (v.verdict ?? v.title).split(/(?<=[.!?])\s/)[0]!.trim()
  return first.length > 120 ? `${first.slice(0, 117).trimEnd()}…` : first
}

/** Where the point is said: the first takeaway inside [start, end), else the start of the stretch. */
function momentOf(v: Card, start: number, end: number): [number, number] {
  const at = (v.takeaways ?? []).map((t) => secs(t.at)).find((s): s is number => s != null && s >= start && s < end - 3)
  const from = at ?? start
  return [from, Math.min(from + MOMENT, end)]
}

/**
 * The reel over the brief: your AI's script when it wrote one, else the picks in order, each with its verdict's first
 * sentence and the moment of its first takeaway. YouTube only: podcast audio can't play inside the app's frame.
 */
export function showOf(data: BriefData): ShowScript | null {
  const byId = new Map(data.videos.map((v) => [v.id, v]))
  const script = data.debrief?.script
  if (script) {
    const clips = script.clips.flatMap((c): Clip[] => {
      const video = byId.get(c.video_id)
      if (!video || video.podcast) return []
      const moment: [number, number] =
        c.moment_start != null && c.moment_end != null && c.moment_end > c.moment_start ? [c.moment_start, c.moment_end] : momentOf(video, c.start, c.end)
      return [{ video, start: c.start, end: c.end, say: c.say, moment }]
    })
    if (clips.length) return { intro: script.intro, clips: clips.slice(0, MAX_CLIPS), outro: script.outro, written: true }
  }
  const clips = data.videos.flatMap((v): Clip[] => {
    const k = kindOf(v)
    const w = worth(v)
    if (v.podcast || !w || (k !== "watch" && k !== "skim")) return []
    return [{ video: v, start: w[0], end: w[1], say: shortLine(v), moment: momentOf(v, w[0], w[1]) }]
  })
  if (!clips.length) return null
  return {
    intro: data.debrief?.headline ?? `${plural(Math.min(clips.length, MAX_CLIPS), "video")} worth your time this week.`,
    clips: clips.slice(0, MAX_CLIPS),
    written: false,
  }
}

/** How long a line stays up: about 2.7 words a second, between 3.5 and 9 s. */
const readMs = (text: string) => Math.min(9000, Math.max(3500, (text.split(/\s+/).length / 2.7) * 1000 + 600))

/** The reel's length in seconds: every line's reading time plus every moment. */
export const reelSeconds = (s: ShowScript) =>
  (readMs(s.intro) + s.clips.reduce((a, c) => a + readMs(c.say) + (c.moment[1] - c.moment[0]) * 1000, 0) + (s.outro ? readMs(s.outro) : 0)) / 1000

/** A caption in pieces of three or four words, the way reels show it. */
function chunks(text: string): string[] {
  const words = text.split(/\s+/).filter(Boolean)
  const out: string[] = []
  for (let i = 0; i < words.length; ) {
    const n = words.length - i <= 5 ? words.length - i : words[i]!.length + (words[i + 1]?.length ?? 0) > 14 ? 3 : 4
    out.push(words.slice(i, i + n).join(" "))
    i += n
  }
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

/** A line runs over its clip at double speed, muted; a moment plays the clip as it is, with sound. */
type Step = { kind: "line"; clip: number; text: string; tag: string; seg: number } | { kind: "moment"; clip: number; seg: number }

/**
 * "Watch your debrief" as a reel: a vertical frame, the videos running fast behind big captions, cut with the few
 * seconds of each where the point is said. Bars on top, one per part; tap the left or right edge to go back or on, the
 * middle to pause.
 */
export function Show({ show, loc, onClose }: { show: ShowScript; loc: Locale; onClose: () => void }) {
  const last = show.clips.length - 1
  const steps = useMemo<Step[]>(
    () => [
      { kind: "line", clip: 0, text: show.intro, tag: "This week", seg: 0 },
      ...show.clips.flatMap((c, i): Step[] => [
        { kind: "line", clip: i, text: c.say, tag: c.video.channel, seg: i + 1 },
        { kind: "moment", clip: i, seg: i + 1 },
      ]),
      ...(show.outro ? [{ kind: "line" as const, clip: last, text: show.outro, tag: "That's the week", seg: show.clips.length + 1 }] : []),
    ],
    [show, last]
  )
  const segments = steps.at(-1)!.seg + 1
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
  const want = useRef({ rate: FAST, muted: true, moment: false })
  const onEvent = useRef<(s: number) => void>(() => {})
  const voices = useVoices()
  const voice = useMemo(() => pickVoice(voices, (loc.locale ?? navigator.language).slice(0, 2).toLowerCase()), [voices, loc.locale])

  const done = at >= steps.length
  const step = done ? undefined : steps[at]
  const clip = step ? show.clips[step.clip] : undefined
  const go = (i: number) => {
    setPaused(false)
    setAt(Math.max(0, Math.min(i, steps.length)))
  }
  const segStart = (seg: number) => steps.findIndex((s) => s.seg === seg)

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
      if (s === PAUSED && !paused && want.current.moment && !want.current.muted) {
        want.current.muted = true
        setMuteForced(true)
        player.current?.mute()
        player.current?.playVideo()
      }
      if (s === ENDED && want.current.moment) setAt((i) => i + 1)
    }
  })

  // Each step: a line loads its video (unless it's already running) fast and muted; a moment jumps to its start, as is.
  useEffect(() => {
    const p = player.current
    if (!ready || !p || !step || !clip) return
    if (step.kind === "line") {
      want.current = { rate: FAST, muted: true, moment: false }
      if (loaded.current !== clip.video.id) {
        loaded.current = clip.video.id
        p.loadVideoById({ videoId: clip.video.id, startSeconds: clip.start })
      } else apply()
    } else {
      want.current = { rate: 1, muted: !sound || muteForced, moment: true }
      p.seekTo(clip.moment[0], true)
      apply()
    }
    if (!paused) p.playVideo()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- re-run on a new step, not on every sound or pause change
  }, [at, ready])

  useEffect(() => {
    if (step?.kind !== "moment") return
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
    if (step?.kind !== "line" || paused || !sound || !voice) return
    const u = new SpeechSynthesisUtterance(step.text)
    u.voice = voice
    u.lang = voice.lang
    u.rate = 1.08
    speechSynthesis.cancel()
    speechSynthesis.speak(u)
    return () => speechSynthesis.cancel()
  }, [step, paused, sound, voice])

  // The clock: a line lasts its reading time; a moment until the player passes its end (or, if it stalls, a few seconds more).
  useEffect(() => {
    if (!step || paused) return
    const length = step.kind === "line" ? readMs(step.text) : (clip!.moment[1] - clip!.moment[0]) * 1000
    const began = performance.now() - frac * length
    let frame = 0
    const tick = (now: number) => {
      let f = (now - began) / length
      if (step.kind === "moment" && player.current && !blocked) {
        const t = player.current.getCurrentTime()
        if (t >= clip!.moment[0] - 1) f = Math.max(f * 0.5, (t - clip!.moment[0]) / (clip!.moment[1] - clip!.moment[0]))
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

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight") go(at + 1)
      else if (e.key === "ArrowLeft") go(step ? segStart(Math.max(0, step.seg - (frac < 0.15 ? 1 : 0))) : 0)
      else if (e.key === " ") setPaused((p) => !p)
      else return
      e.preventDefault()
      setFrac(0)
    }
    addEventListener("keydown", onKey)
    return () => removeEventListener("keydown", onKey)
  })

  const tap = (where: "back" | "pause" | "on") => {
    if (where === "pause") return setPaused((p) => !p)
    setFrac(0)
    if (where === "on") go(at + 1)
    else go(step ? segStart(Math.max(0, step.seg - 1)) : segStart(segments - 1))
  }

  const line = step?.kind === "line" ? step : null
  const pieces = line ? chunks(line.text) : []
  const piece = pieces[Math.min(pieces.length - 1, Math.floor(frac * pieces.length))]

  return (
    <div className="mx-auto flex max-w-[980px] flex-col items-center gap-3 px-4 pt-3 pb-6">
      <div className="flex w-full items-center justify-between gap-3">
        <Button variant="ghost" size="sm" className="-ml-2.5 font-semibold text-muted-foreground" onClick={onClose}>
          <ArrowLeftIcon data-icon="inline-start" />
          Your brief
        </Button>
        <ExpandButton />
      </div>

      <div className="relative aspect-[9/16] h-[min(calc(var(--app-h)-96px),880px)] overflow-hidden rounded-2xl bg-black text-white shadow-2xl ring-1 ring-white/10">
        {/* The video, cropped to the frame; it takes no clicks, the tap zones do. */}
        <div className="pointer-events-none absolute inset-y-0 left-1/2 aspect-video h-full -translate-x-1/2 [&>iframe]:size-full">
          <div ref={holder} />
        </div>
        {blocked && clip?.video.thumbnail && (
          <img src={clip.video.thumbnail} alt="" className="reel-pan absolute inset-0 size-full object-cover" key={clip.video.id} />
        )}

        <div
          aria-hidden
          className={cn(
            "pointer-events-none absolute inset-0 transition-opacity duration-300",
            line || done ? "bg-[linear-gradient(to_top,rgb(0_0_0/.85),rgb(0_0_0/.35)_55%,rgb(0_0_0/.25))] opacity-100" : "bg-[linear-gradient(to_top,rgb(0_0_0/.7),transparent_35%)] opacity-100"
          )}
        />

        {/* One bar per part: filled, filling, to come. */}
        <div className="absolute inset-x-3 top-3 flex gap-1" aria-hidden>
          {Array.from({ length: segments }, (_, s) => {
            const mine = step?.seg === s
            const within = mine ? (steps.filter((x) => x.seg === s).indexOf(step!) + frac) / steps.filter((x) => x.seg === s).length : 0
            return (
              <div key={s} className="h-[3px] flex-1 overflow-hidden rounded-full bg-white/25">
                <div className="h-full bg-white" style={{ width: `${done || (step && s < step.seg) ? 100 : mine ? within * 100 : 0}%` }} />
              </div>
            )
          })}
        </div>

        {line && (
          <>
            <div className="absolute inset-x-4 top-7 flex items-center gap-2 text-[12px] font-semibold text-white/85">
              <span className="truncate">{line.tag}</span>
              {!blocked && <span className="rounded bg-white/20 px-1.5 py-px font-mono text-[11px]">{FAST}×</span>}
            </div>
            <p key={`${at}-${piece}`} className="reel-word absolute inset-x-5 bottom-[30%] m-0 text-center font-slate text-[clamp(26px,8cqh,44px)] leading-[1.05] text-balance [text-shadow:0_2px_12px_rgb(0_0_0/.6)]">
              {piece}
            </p>
          </>
        )}

        {step?.kind === "moment" && clip && (
          <div className="absolute inset-x-4 bottom-5 grid gap-1.5">
            {blocked ? (
              <ExtLink url={`https://www.youtube.com/watch?v=${clip.video.id}&t=${clip.moment[0]}s`} className="relative z-20 text-[15px] font-semibold underline">
                Play this moment on YouTube
              </ExtLink>
            ) : (
              <span className="flex items-center gap-1.5 text-[12px] font-semibold text-white/85">
                <span className="rounded bg-white/20 px-1.5 py-px font-mono text-[11px]">{clock(clip.moment[0])}</span>
                {muteForced ? "Muted: this app doesn't let it play sound" : "As it is"}
              </span>
            )}
            <span className="line-clamp-2 text-[15px] leading-snug font-semibold">{clip.video.title}</span>
            <span className="text-[12.5px] text-white/70">{clip.video.channel}</span>
          </div>
        )}

        {done && (
          <div className="absolute inset-0 z-20 grid place-content-center justify-items-center gap-4 px-8 text-center">
            <p className="m-0 font-slate text-[32px] leading-[1.05]">That's your debrief.</p>
            <div className="flex gap-2">
              <Button variant="secondary" onClick={() => go(0)}>
                <RotateCcwIcon data-icon="inline-start" />
                Watch again
              </Button>
              <Button variant="ghost" className="text-white/80 hover:bg-white/10 hover:text-white" onClick={onClose}>
                Back to your brief
              </Button>
            </div>
          </div>
        )}

        {!done && (
          <div className="absolute inset-x-0 top-10 bottom-0 z-10 grid grid-cols-[30%_1fr_30%]">
            <button type="button" aria-label="Back" className="cursor-pointer" onClick={() => tap("back")} />
            <button type="button" aria-label={paused ? "Play" : "Pause"} className="grid cursor-pointer place-items-center" onClick={() => tap("pause")}>
              {paused && (
                <span className="grid size-16 place-items-center rounded-full bg-black/50">
                  <PlayIcon className="size-7" />
                </span>
              )}
            </button>
            <button type="button" aria-label="Next" className="cursor-pointer" onClick={() => tap("on")} />
          </div>
        )}

        {!done && (
          <button
            type="button"
            aria-label={sound ? "Sound off" : "Sound on"}
            title={voice ? `Narrator: ${voice.name}` : "No voice on this computer: the captions carry the narration"}
            onClick={() => setSound((s) => !s)}
            className="absolute top-7 right-3 z-20 grid size-8 cursor-pointer place-items-center rounded-full bg-black/40 hover:bg-black/60"
          >
            {sound ? <Volume2Icon className="size-4" /> : <VolumeXIcon className="size-4" />}
          </button>
        )}
      </div>

      <p className="m-0 max-w-[60ch] text-center text-[12px] text-faint">
        {plural(show.clips.length, "video")}, each fast-forwarded under its line, then the moment that matters.
        {!show.written && " Made from the picks' verdicts; after the next screening your AI writes the lines."}
      </p>
    </div>
  )
}
