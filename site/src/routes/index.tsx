import { createFileRoute } from "@tanstack/react-router"
import { GithubIcon } from "@/components/icons"
import { InstallTabs } from "@/components/install-tabs"
import { SiteFooter } from "@/components/site-footer"
import { SiteHeader } from "@/components/site-header"
import { ReviewTerminal } from "@/components/terminal"
import { Button } from "@/components/ui/button"
import { REPO_URL, VERSION } from "@/lib/site"

export const Route = createFileRoute("/")({ component: Home })

/** Source of every claim: README.md, install.sh, install.ps1, LICENSE. */
const STEPS = [
  ["Link", "Give tx a video or podcast link."],
  [
    "Transcript",
    "tx gets the captions, or transcribes the audio on your computer.",
  ],
  ["Verdict", "Your Claude or Codex writes the verdict and takeaways."],
]

const GET = [
  [
    "text-ok",
    "WATCH · SKIM · SKIP",
    "A verdict and a 0–5 relevance score for you, not for the video in general.",
  ],
  [
    "text-amber",
    "Timeline",
    "The part worth watching, drawn on a bar when the source gives a duration.",
  ],
  [
    "text-cyan",
    "Takeaways",
    "Timestamped, with a TL;DR. Ctrl+click a timestamp to open that moment, in terminals that support OSC 8 links.",
  ],
  [
    "text-foreground",
    "Markdown",
    "Ask your own question and save the answer as a note.",
  ],
]

const COMMANDS = [
  ["Review one link", 'tx "URL" -q'],
  ["Triage several tabs", 'tx "URL1" "URL2" "URL3" -q'],
  ["Decide whether to follow a channel", "tx channel @t3dotgg --top 5"],
  ["Screen new uploads from sources you follow", "tx run"],
  [
    "Ask your own question, save a note",
    'tx "URL" -a "Explain the technique with timestamps and the source URL" > note.md',
  ],
]

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

function Split({
  label,
  title,
  id,
  children,
}: {
  label: string
  title: string
  id?: string
  children: React.ReactNode
}) {
  return (
    <section id={id} className="scroll-mt-14 border-t py-16 sm:py-24">
      <div className="wrap grid gap-8 lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)] lg:gap-16">
        <div>
          <p className="eyebrow">{label}</p>
          <h2 className="h2">{title}</h2>
        </div>
        <div className="min-w-0">{children}</div>
      </div>
    </section>
  )
}

function WhatYouGet() {
  return (
    <Split label="What you get" title="A review you can act on.">
      <dl className="divide-y border-y">
        {GET.map(([tone, term, text]) => (
          <div
            key={term}
            className="grid gap-1.5 py-5 sm:grid-cols-[13rem_1fr] sm:gap-8"
          >
            <dt className={`font-mono text-[13px] font-bold ${tone}`}>
              {term}
            </dt>
            <dd className="text-[15px] text-muted-foreground">{text}</dd>
          </div>
        ))}
      </dl>
    </Split>
  )
}

function Commands() {
  return (
    <Split label="Commands" title="Start with one link.">
      <ul className="divide-y border-y">
        {COMMANDS.map(([label, cmd]) => (
          <li
            key={label}
            className="grid gap-2 py-5 sm:grid-cols-[minmax(0,15rem)_minmax(0,1fr)] sm:gap-8"
          >
            <span className="text-[15px] font-medium">{label}</span>
            <code className="block font-mono text-[13px] leading-relaxed break-words text-foreground">
              <span aria-hidden="true" className="mr-2 text-cyan select-none">
                $
              </span>
              {cmd}
            </code>
          </li>
        ))}
      </ul>
      <p className="mt-4 font-mono text-[11px] text-dim">
        Replace URL with a quoted link.
      </p>
    </Split>
  )
}

function HowItWorks() {
  return (
    <section id="how" className="border-t py-16 sm:py-24">
      <div className="wrap">
        <p className="eyebrow">How it works</p>
        <ol className="grid gap-8 sm:grid-cols-3 sm:gap-10">
          {STEPS.map(([tag, text], i) => (
            <li key={tag} className="border-t border-border-strong pt-4">
              <span className="font-mono text-[11px] text-cyan uppercase">
                {i + 1} · {tag}
              </span>
              <p className="mt-2.5 max-w-[22em] font-heading text-xl leading-snug font-semibold tracking-[-0.02em]">
                {text}
              </p>
            </li>
          ))}
        </ol>
      </div>
    </section>
  )
}

function Requirements() {
  return (
    <section id="requirements" className="border-t py-16 sm:py-24">
      <div className="wrap">
        <p className="max-w-[48em] text-[15px] leading-relaxed text-muted-foreground">
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
          , signed in. Runs on macOS, Linux and Windows. tx runs on your
          computer: no tx account, no hosted tx service.
        </p>
      </div>
    </section>
  )
}
