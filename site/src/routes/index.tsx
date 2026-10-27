import * as React from "react"
import { createFileRoute } from "@tanstack/react-router"
import { GithubIcon } from "@/components/icons"
import { InstallTabs } from "@/components/install-tabs"
import { SiteFooter } from "@/components/site-footer"
import { SiteHeader } from "@/components/site-header"
import { ReviewTerminal } from "@/components/terminal"
import { Button } from "@/components/ui/button"
import { REPO_URL, VERSION } from "@/lib/site"

export const Route = createFileRoute("/")({ component: Home })

/** Source of every claim: README.md, install.sh, install.ps1, LICENSE, .github/assets/tx-tutorial.jpg. */
const STEPS = [
  ["link", "Paste a video or podcast link."],
  [
    "transcript",
    "tx fetches the captions, or transcribes the audio on your computer.",
  ],
  ["verdict", "Your own Claude or Codex writes the verdict and the takeaways."],
] as const

const COMMANDS = [
  ["Review one link", 'tx "URL" -q'],
  ["Triage several tabs", 'tx "URL1" "URL2" "URL3" -q'],
  ["Decide whether to follow a channel", "tx channel @t3dotgg --top 5"],
  ["Screen new uploads from sources you follow", "tx run"],
  [
    "Ask your own question, save a note",
    'tx "URL" -a "Explain the technique with timestamps and the source URL" > note.md',
  ],
] as const

type Tone = "watch" | "skim" | "skip"

const TONE: Record<Tone, string> = {
  watch: "bg-ok-fill",
  skim: "bg-amber-fill",
  skip: "bg-coral-fill",
}

/** The verdict badge, drawn like tx draws it in the terminal. */
function Verdict({ tone, className }: { tone: Tone; className?: string }) {
  return (
    <span
      className={`inline-block px-2 py-0.5 font-mono text-[13px] font-bold tracking-wide text-ink uppercase ${TONE[tone]} ${className ?? ""}`}
    >
      {tone}
    </span>
  )
}

const inlineLink =
  "text-cyan underline decoration-cyan/40 underline-offset-4 transition-colors hover:decoration-cyan"

function Home() {
  return (
    <>
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-lg focus:bg-foreground focus:px-3 focus:py-2 focus:text-background"
      >
        Skip to content
      </a>
      <SiteHeader />
      <main id="main">
        <Hero />
        <WhatYouGet />
        <Commands />
        <HowItWorks />
        <Requirements />
      </main>
      <SiteFooter />
    </>
  )
}

function Hero() {
  const d = (n: number) => ({ "--d": n }) as React.CSSProperties
  return (
    <section className="pt-14 pb-20 sm:pt-20 sm:pb-28 lg:pt-24 lg:pb-32">
      <div className="wrap grid items-center gap-14 lg:grid-cols-2 lg:gap-16 2xl:gap-24">
        <div className="min-w-0">
          <p className="eyebrow rise">v{VERSION} · Open source</p>
          <h1 className="h1 rise" style={d(1)}>
            Is this video worth watching?
          </h1>
          <p className="lede rise" style={d(2)}>
            Paste a link. Your own Claude or Codex gives you a{" "}
            <b className="font-semibold text-ok">WATCH</b>,{" "}
            <b className="font-semibold text-amber">SKIM</b> or{" "}
            <b className="font-semibold text-coral">SKIP</b> verdict and
            timestamped takeaways.
          </p>

          <div className="rise mt-9" style={d(3)}>
            <InstallTabs />
          </div>

          <div className="rise mt-6" style={d(4)}>
            <Button
              variant="outline"
              size="pill"
              nativeButton={false}
              render={<a href={REPO_URL} />}
              className="dark:bg-transparent"
            >
              <GithubIcon />
              View on GitHub
            </Button>
          </div>
        </div>

        <div className="rise min-w-0" style={d(3)}>
          <ReviewTerminal />
          <p className="mt-4 text-center font-mono text-[11px] text-dim">
            Real tx 0.20.0 output, example profile.
          </p>
        </div>
      </div>
    </section>
  )
}

/** A section of the page, sorted the way tx sorts a video. */
function Chapter({
  tone,
  label,
  title,
  id,
  split = false,
  children,
}: {
  tone: Tone
  label: string
  title?: string
  id?: string
  /** Heading on the left, content on the right, on wide screens. */
  split?: boolean
  children: React.ReactNode
}) {
  return (
    <section id={id} className="scroll-mt-14 border-t py-20 sm:py-28">
      <div
        className={
          split
            ? "wrap grid gap-10 lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)] lg:gap-16"
            : "wrap"
        }
      >
        <div
          className={
            split ? "flex flex-col gap-4" : "mb-10 flex flex-col gap-4 sm:mb-14"
          }
        >
          <p className="flex items-center gap-3 font-mono text-xs text-muted-foreground uppercase">
            <Verdict tone={tone} />
            <span>{label}</span>
          </p>
          {title ? <h2 className="h2 mb-0">{title}</h2> : null}
        </div>
        <div className="min-w-0">{children}</div>
      </div>
    </section>
  )
}

function Tile({
  title,
  text,
  children,
}: {
  title: string
  text: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <div className="flex flex-col gap-6 rounded-2xl bg-card p-7 shadow-(--shadow-card) sm:p-9">
      <div className="flex min-h-24 items-center">{children}</div>
      <div>
        <h3 className="font-heading text-xl font-semibold tracking-[-0.02em]">
          {title}
        </h3>
        <p className="mt-2 max-w-[34em] text-[15px] leading-relaxed text-muted-foreground">
          {text}
        </p>
      </div>
    </div>
  )
}

function WhatYouGet() {
  // Real example from tx-tutorial.jpg: SKIM 4/5, skim 01:47–04:44 of a 05:03 video.
  const start = (107 / 303) * 100
  const width = ((284 - 107) / 303) * 100
  return (
    <Chapter
      tone="watch"
      label="What you get"
      title="One link in, a decision out."
      id="what-you-get"
    >
      <div className="grid gap-5 md:grid-cols-2">
        <Tile
          title="A verdict for you"
          text="WATCH, SKIM or SKIP, with a 0–5 relevance score for your interests, not for the video in general."
        >
          <div className="flex flex-wrap items-center gap-3">
            <Verdict tone="watch" className="text-base" />
            <Verdict tone="skim" className="text-base" />
            <Verdict tone="skip" className="text-base" />
            <span className="ml-2 font-mono text-base text-ok">●●●●○ 4/5</span>
          </div>
        </Tile>
        <Tile
          title="The part worth watching"
          text="Drawn on a timeline when the source gives a duration, so you can jump straight there."
        >
          <div className="w-full font-mono text-[13px]">
            <div className="term-bar-hatch relative h-4 w-full">
              <div
                className="term-bar-fill absolute inset-y-0"
                style={{ left: `${start}%`, width: `${width}%` }}
              />
            </div>
            <div className="mt-2 flex justify-between text-cyan">
              <span>00:00</span>
              <span className="text-amber">01:47–04:44</span>
              <span>05:03</span>
            </div>
          </div>
        </Tile>
        <Tile
          title="Timestamped takeaways"
          text="Each takeaway points to its moment, with a TL;DR on top. Ctrl+click a timestamp to open the video right there, in terminals that support OSC 8 links."
        >
          <p className="font-mono text-[15px] leading-relaxed">
            <span className="font-bold text-amber">skim</span>
            <span className="text-muted-foreground"> · </span>
            <span className="text-cyan underline decoration-cyan/40 underline-offset-4">
              01:47–04:44
            </span>
            <br />
            <span className="text-muted-foreground">
              This part covers why current defenses fail…
            </span>
          </p>
        </Tile>
        <Tile
          title="Notes you keep"
          text="Ask your own question about the video and save the answer as Markdown, wherever you keep notes."
        >
          <p className="font-mono text-[15px]">
            <span className="text-cyan">$</span> tx "URL" -a "…"{" "}
            <span className="text-amber">&gt; note.md</span>
          </p>
        </Tile>
      </div>
    </Chapter>
  )
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = React.useState(false)
  return (
    <button
      type="button"
      onClick={() => {
        void navigator.clipboard.writeText(text).then(() => {
          setCopied(true)
          window.setTimeout(() => setCopied(false), 1400)
        })
      }}
      className="shrink-0 rounded-md px-2 py-1 text-[11px] text-(--t-grey) opacity-0 transition-opacity group-hover:opacity-100 hover:text-(--t-fg) focus-visible:opacity-100"
      aria-label={`Copy ${text}`}
    >
      {copied ? "Copied" : "Copy"}
    </button>
  )
}

function Commands() {
  return (
    <Chapter
      tone="skim"
      label="Commands"
      title="Five commands cover it."
      id="commands"
      split
    >
      <figure className="term overflow-hidden rounded-2xl font-mono text-[13px] leading-relaxed shadow-(--shadow-term) sm:text-[14px]">
        <div
          className="flex items-center gap-2 px-4 py-3"
          style={{ background: "var(--t-bar)" }}
        >
          <span className="size-2.5 rounded-full bg-[#ff5f57]" />
          <span className="size-2.5 rounded-full bg-[#febc2e]" />
          <span className="size-2.5 rounded-full bg-[#28c840]" />
        </div>
        <ul className="flex flex-col gap-5 px-5 py-6 sm:px-8 sm:py-8">
          {COMMANDS.map(([label, cmd]) => (
            <li key={label} className="group">
              <p className="text-(--t-grey)"># {label}</p>
              <div className="flex items-start justify-between gap-4">
                <code className="min-w-0 break-words">
                  <span
                    aria-hidden="true"
                    className="mr-2 text-(--t-cyan) select-none"
                  >
                    $
                  </span>
                  {cmd}
                </code>
                <CopyButton text={cmd} />
              </div>
            </li>
          ))}
          <li className="text-(--t-grey)"># Replace URL with a quoted link.</li>
        </ul>
      </figure>
    </Chapter>
  )
}

function HowItWorks() {
  return (
    <Chapter tone="skim" label="How it works" id="how">
      <ol className="grid gap-10 md:grid-cols-3 md:gap-0">
        {STEPS.map(([node, text], i) => (
          <li key={node} className="relative md:pr-10">
            <div className="flex items-center gap-3">
              <span className="rounded-full border border-border-strong px-4 py-1.5 font-mono text-sm">
                {node}
              </span>
              {i < STEPS.length - 1 ? (
                <span
                  aria-hidden="true"
                  className="hidden h-px flex-1 bg-border-strong md:block"
                />
              ) : null}
              {i < STEPS.length - 1 ? (
                <span
                  aria-hidden="true"
                  className="hidden font-mono text-sm text-dim md:inline"
                >
                  →
                </span>
              ) : null}
            </div>
            <p className="mt-5 max-w-[20em] font-heading text-xl leading-snug font-semibold tracking-[-0.02em]">
              {text}
            </p>
          </li>
        ))}
      </ol>
    </Chapter>
  )
}

function Requirements() {
  return (
    <Chapter tone="skip" label="The fine print" id="requirements">
      <p className="max-w-[48em] text-[17px] leading-relaxed text-muted-foreground">
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
        , signed in. Runs on macOS, Linux and Windows. tx runs on your computer:
        no tx account, no hosted tx service.
      </p>
    </Chapter>
  )
}
