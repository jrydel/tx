import * as React from "react"
import { createFileRoute } from "@tanstack/react-router"
import { InstallTabs } from "@/components/install-tabs"
import { SiteFooter } from "@/components/site-footer"
import { SiteHeader } from "@/components/site-header"
import { cn } from "@/lib/utils"
import { VERSION } from "@/lib/site"

export const Route = createFileRoute("/")({ component: Home })

/** Chapters of the README tutorial (tx-terminal.mp4), read from its own captions; times in seconds. */
const DURATION = 74.5
const CHAPTERS = [
  { start: 0, label: "Install" },
  { start: 3, label: "Set up" },
  { start: 21, label: "First screening" },
  { start: 30, label: "Review one link" },
  { start: 39, label: "Review several links" },
  { start: 57, label: "Sample a channel" },
] as const

const stamp = (s: number) =>
  `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(Math.floor(s % 60)).padStart(2, "0")}`

const inlineLink =
  "text-cyan underline decoration-cyan/40 underline-offset-4 transition-colors hover:decoration-cyan"

function Home() {
  const d = (n: number) => ({ "--d": n }) as React.CSSProperties
  return (
    <>
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-lg focus:bg-foreground focus:px-3 focus:py-2 focus:text-background"
      >
        Skip to content
      </a>
      <SiteHeader />
      <main
        id="main"
        className="wrap flex flex-col items-center gap-10 pt-10 pb-16 sm:pt-14 sm:pb-20"
      >
        <section className="flex w-full max-w-[640px] flex-col items-center gap-4 text-center">
          <p className="rise font-mono text-[11px] text-muted-foreground">
            tx {VERSION} · open source
          </p>
          <h1
            className="rise font-heading text-[clamp(26px,3.2vw,36px)] leading-[1.08] font-bold tracking-[-0.035em]"
            style={d(1)}
          >
            Is this video worth watching?
          </h1>
          <p
            className="rise max-w-[30em] text-[15px] leading-relaxed text-muted-foreground"
            style={d(2)}
          >
            Paste a video or podcast link. Your own Claude or Codex answers{" "}
            <b className="font-semibold text-ok">WATCH</b>,{" "}
            <b className="font-semibold text-amber">SKIM</b> or{" "}
            <b className="font-semibold text-coral">SKIP</b>, with timestamped
            takeaways.
          </p>
          <div
            className="rise mt-2 flex w-full flex-col items-center gap-2 text-left"
            style={d(3)}
          >
            <InstallTabs className="w-full min-w-0" />
            <p className="text-center text-[12px] leading-relaxed text-muted-foreground">
              Needs{" "}
              <a
                className={inlineLink}
                href="https://code.claude.com/docs/en/quickstart"
              >
                Claude Code
              </a>{" "}
              or{" "}
              <a
                className={inlineLink}
                href="https://learn.chatgpt.com/docs/codex/cli"
              >
                Codex CLI
              </a>
              , signed in. Runs on your computer, no tx account.
            </p>
          </div>
        </section>

        <section
          aria-label="Tutorial"
          className="rise w-full max-w-[920px]"
          style={d(4)}
        >
          <TutorialPlayer />
        </section>
      </main>
      <SiteFooter />
    </>
  )
}

/**
 * The README tutorial (release asset tx-terminal.mp4, re-encoded to 720p; silent, with captions) with a chapter
 * timeline drawn like tx's own: the part playing is filled amber, and every timestamp jumps the video there.
 * Plays muted on a loop; stays paused when the visitor asks for reduced motion.
 */
function TutorialPlayer() {
  const ref = React.useRef<HTMLVideoElement>(null)
  const [time, setTime] = React.useState(0)

  React.useEffect(() => {
    const video = ref.current
    if (!video) return
    const onTime = () => setTime(video.currentTime)
    video.addEventListener("timeupdate", onTime)
    if (!window.matchMedia("(prefers-reduced-motion: reduce)").matches)
      void video.play().catch(() => undefined)
    return () => video.removeEventListener("timeupdate", onTime)
  }, [])

  const seek = (start: number) => {
    const video = ref.current
    if (!video) return
    video.currentTime = start + 0.01
    void video.play().catch(() => undefined)
  }

  const active = CHAPTERS.reduce(
    (index, chapter, i) => (time >= chapter.start ? i : index),
    0
  )

  return (
    <figure className="flex flex-col gap-4">
      <video
        ref={ref}
        className="aspect-video w-full rounded-xl border border-border bg-black"
        src="/tx-tutorial.mp4"
        poster="/tx-tutorial-poster.jpg"
        width={1280}
        height={720}
        muted
        loop
        playsInline
        controls
        preload="metadata"
        aria-label="tx tutorial: install, setup, then reviewing videos in the terminal"
      />

      <div className="flex gap-0.5" aria-hidden="true">
        {CHAPTERS.map((chapter, i) => {
          const end = i + 1 < CHAPTERS.length ? CHAPTERS[i + 1].start : DURATION
          const length = end - chapter.start
          const played = Math.min(
            1,
            Math.max(0, (time - chapter.start) / length)
          )
          return (
            <button
              key={chapter.label}
              type="button"
              tabIndex={-1}
              onClick={() => seek(chapter.start)}
              className="hatch relative h-2.5 cursor-pointer overflow-hidden"
              style={{ flexGrow: length, flexBasis: 0 }}
            >
              <span
                className="absolute inset-y-0 left-0 bg-amber-fill transition-[width] duration-300 ease-linear"
                style={{ width: `${played * 100}%` }}
              />
            </button>
          )
        })}
      </div>

      <figcaption className="flex flex-col gap-3">
        <ol className="grid grid-cols-2 gap-x-6 gap-y-1.5 font-mono text-[12px] sm:grid-cols-3">
          {CHAPTERS.map((chapter, i) => (
            <li key={chapter.label}>
              <button
                type="button"
                onClick={() => seek(chapter.start)}
                aria-label={`Play from ${stamp(chapter.start)}: ${chapter.label}`}
                className={cn(
                  "flex items-baseline gap-2 text-left transition-colors",
                  i === active
                    ? "text-foreground"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                <span className="text-cyan underline decoration-cyan/40 underline-offset-4">
                  {stamp(chapter.start)}
                </span>
                <span className={cn(i === active && "font-semibold")}>
                  {chapter.label}
                </span>
              </button>
            </li>
          ))}
        </ol>
        <p className="font-mono text-[11px] text-dim">
          Tutorial · 01:14 · silent, with captions · real tx 0.20.0 output
        </p>
      </figcaption>
    </figure>
  )
}
