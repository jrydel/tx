import { createFileRoute } from "@tanstack/react-router"
import { ArrowRight, ArrowUpRight } from "lucide-react"
import { CodeBlock } from "@/components/code-block"
import { GithubIcon } from "@/components/icons"
import { InstallTabs } from "@/components/install-tabs"
import { SiteFooter } from "@/components/site-footer"
import { SiteHeader } from "@/components/site-header"
import { ReviewTerminal } from "@/components/terminal"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import {
  EXAMPLE_URL,
  LICENSE_URL,
  RELEASES_URL,
  REPO_URL,
  TUTORIAL_URL,
  VERSION,
} from "@/lib/site"

export const Route = createFileRoute("/")({ component: Home })

/** Source of every claim: README.md unless noted. */
const USE_CASES = [
  {
    title: "Review a link",
    text: "Get a WATCH / SKIM / SKIP verdict, a relevance score, the part worth watching, and timestamped takeaways.",
    cmd: 'tx "URL" -q',
  },
  {
    title: "Choose what to follow",
    text: "Get a YouTube channel report based on videos selected for your interests.",
    cmd: "tx channel @t3dotgg --top 5",
  },
  {
    title: "Keep up without checking every channel",
    text: "Get a daily brief of new uploads, scored against your profile.",
    cmd: "tx run",
  },
  {
    title: "Keep what you learn",
    text: "Ask your own question and save the answer as a Markdown note.",
    cmd: 'tx "URL" -a "Explain the technique with timestamps and the source URL" > note.md',
  },
]

const COMMANDS = [
  ["Someone sends you a video", 'tx "URL" -q'],
  ["You have several tabs to triage", 'tx "URL1" "URL2" "URL3" -q'],
  [
    "Deciding whether to follow a YouTube channel",
    "tx channel @t3dotgg --top 5",
  ],
  [
    "You want a reusable note",
    'tx "URL" -a "Explain the technique with timestamps and the source URL" > note.md',
  ],
  ["You want new uploads from sources you follow", "tx run"],
]

const STEPS = [
  {
    tag: "Link",
    title: "You give it a link",
    text: "A video or podcast URL. tx uses yt-dlp, so it handles links from many video and audio sites.",
  },
  {
    tag: "Transcript",
    title: "tx gets the transcript",
    text: "It tries platform captions first. Without captions, it downloads the audio and transcribes it on your computer with Whisper. Transcripts are cached.",
  },
  {
    tag: "Analysis",
    title: "Your agent reads it",
    text: "Claude Code or Codex analyzes the transcript through your own account. tx picks Claude first, then Codex, unless you choose with -g.",
  },
  {
    tag: "Note",
    title: "You get a review",
    text: "A verdict, a 0–5 relevance score, timestamped takeaways and a TL;DR. Save any answer as a Markdown note with >.",
    live: true,
  },
]

const SOURCES: Array<[string, string, boolean]> = [
  [
    "YouTube",
    "Real AI review and local-note workflows using a cached transcript",
    true,
  ],
  [
    "InfoQ talk",
    "Downloaded and locally transcribed the full 48-minute talk",
    true,
  ],
  [
    "Apple Podcasts",
    "Downloaded and locally transcribed a 90-second episode",
    true,
  ],
  [
    "Vimeo",
    "Tested page required login; public-player attempt failed on a DRM-protected format",
    false,
  ],
]

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
        <Facts />
        <WhatItDoes />
        <StartSmall />
        <HowItWorks />
        <QuickStart />
        <SourcesPrivacy />
        <GoFurther />
        <Requirements />
        <FinalCta />
      </main>
      <SiteFooter />
    </>
  )
}

function Hero() {
  return (
    <section className="pt-12 pb-16 sm:pt-19 sm:pb-21">
      <div className="wrap grid items-center gap-12 lg:grid-cols-[1.1fr_0.9fr] lg:gap-14">
        <div className="min-w-0">
          <p className="eyebrow rise">v{VERSION} · Open source · MIT</p>
          <h1 className="h1 rise" style={{ "--d": 1 } as React.CSSProperties}>
            Turn videos and podcasts into knowledge you keep.
          </h1>
          <p className="lede rise" style={{ "--d": 2 } as React.CSSProperties}>
            Paste a video or podcast link. tx tells you if it is worth your
            time, with a WATCH, SKIM or SKIP verdict and timestamped takeaways
            from your own Claude or Codex. Save the result as Markdown.
          </p>

          <div
            className="rise mt-7 max-w-[34rem]"
            style={{ "--d": 3 } as React.CSSProperties}
          >
            <InstallTabs />
          </div>

          <div
            className="rise mt-5 flex flex-wrap gap-2.5"
            style={{ "--d": 4 } as React.CSSProperties}
          >
            <Button
              size="pill"
              nativeButton={false}
              render={<a href={REPO_URL} />}
              className="max-sm:flex-1"
            >
              <GithubIcon />
              View on GitHub
            </Button>
            <Button
              variant="outline"
              size="pill"
              nativeButton={false}
              render={<a href={TUTORIAL_URL} />}
              className="max-sm:flex-1 dark:bg-transparent"
            >
              Watch the tutorial
            </Button>
          </div>

          <p
            className="rise mt-4.5 flex items-center gap-2 text-[13px] text-muted-foreground before:h-px before:w-4.5 before:shrink-0 before:bg-border-strong"
            style={{ "--d": 5 } as React.CSSProperties}
          >
            Runs on your computer. No tx account, no hosted tx service.
          </p>
        </div>

        <div
          className="rise min-w-0"
          style={{ "--d": 3 } as React.CSSProperties}
        >
          <ReviewTerminal />
          <p className="mt-3.5 text-center font-mono text-[11px] text-dim">
            Real tx 0.20.0 output, example profile.
          </p>
        </div>
      </div>
    </section>
  )
}

function Facts() {
  const facts = [
    ["WATCH · SKIM · SKIP", "A verdict for every link"],
    ["0–5", "Relevance to you, not the video's objective quality"],
    ["3 OS", "macOS, Linux and Windows"],
    ["MIT", "Open source"],
  ]
  return (
    <section aria-label="Facts" className="border-y">
      <div className="wrap grid grid-cols-2 md:grid-cols-4">
        {facts.map(([num, label], i) => (
          <div
            key={num}
            className="flex flex-col gap-1.5 px-3 py-5 not-first:border-l first:pl-0 max-md:nth-3:border-l-0 max-md:nth-3:pl-0 max-md:nth-[n+3]:border-t md:py-6.5 md:pr-5 md:pl-5 md:first:pl-0"
          >
            <span
              className={
                i === 0
                  ? "font-mono text-[15px] font-semibold tracking-tight md:py-0.5 md:text-base"
                  : "text-[22px] leading-8 font-semibold tracking-[-0.03em]"
              }
            >
              {num}
            </span>
            <span className="font-mono text-[11px] text-dim uppercase">
              {label}
            </span>
          </div>
        ))}
      </div>
    </section>
  )
}

function SectionHead({
  label,
  title,
  children,
}: {
  label: string
  title: string
  children?: React.ReactNode
}) {
  return (
    <div className="grid items-end gap-5 md:grid-cols-2 md:gap-14">
      <div>
        <p className="eyebrow">{label}</p>
        <h2 className="h2 mb-0">{title}</h2>
      </div>
      {children ? (
        <p className="max-w-[34em] text-[16.5px] text-muted-foreground">
          {children}
        </p>
      ) : null}
    </div>
  )
}

function WhatItDoes() {
  return (
    <section className="section">
      <div className="wrap">
        <SectionHead
          label="01 / What it does"
          title="Know what is worth your time."
        >
          tx helps you decide what is worth watching for you, not just get a
          shorter version of a video.
        </SectionHead>
        <div className="mt-11 grid gap-4 sm:grid-cols-2">
          {USE_CASES.map((u) => (
            <Card
              key={u.title}
              className="gap-4 bg-card py-5 shadow-none ring-border"
            >
              <CardContent className="flex h-full flex-col gap-3 px-5">
                <h3 className="text-[15px] font-semibold tracking-[-0.01em]">
                  {u.title}
                </h3>
                <p className="flex-1 text-[14.5px] text-muted-foreground">
                  {u.text}
                </p>
                <code className="mt-1 block rounded-lg border bg-muted px-3 py-2 font-mono text-xs break-words text-foreground">
                  <span
                    aria-hidden="true"
                    className="mr-2 text-amber select-none"
                  >
                    $
                  </span>
                  {u.cmd}
                </code>
              </CardContent>
            </Card>
          ))}
        </div>
      </div>
    </section>
  )
}

function StartSmall() {
  const checks = [
    "Start with one video. If the result is useful, add a profile. Follow a few sources and enable a schedule only if you want an ongoing brief.",
    "Without a profile, tx judges videos for a curious technical generalist. Run tx onboard to describe yourself, or tx learn to learn it from your coding-agent sessions.",
    "Click to watch: in a terminal that supports OSC 8 hyperlinks, Ctrl+click a takeaway timestamp to open that moment in your browser.",
    "The score measures relevance to you, not the video's objective quality. Check important claims against the linked source.",
  ]
  return (
    <section className="section border-y bg-muted">
      <div className="wrap grid items-center gap-12 lg:grid-cols-[0.9fr_1.1fr] lg:gap-14">
        <div>
          <p className="eyebrow">02 / Which command</p>
          <h2 className="h2">Pick the command that fits the moment.</h2>
          <p className="lede">
            Single links, several tabs, whole channels, new uploads. Replace URL
            with a quoted link.
          </p>
          <ul className="mt-6 grid gap-2.5 text-[14.5px]">
            {checks.map((c) => (
              <li
                key={c}
                className="relative pl-6.5 before:absolute before:top-[7px] before:left-0 before:size-3.5 before:rounded-sm before:border before:border-border-strong before:bg-[linear-gradient(var(--amber-fill),var(--amber-fill))] before:bg-size-[6px_6px] before:bg-center before:bg-no-repeat"
              >
                {c}
              </li>
            ))}
          </ul>
        </div>

        <div className="min-w-0 rounded-2xl bg-card p-4 shadow-(--shadow-card)">
          <div className="mb-1 flex justify-between border-b pb-3 font-mono text-[11.5px]">
            <span>Which command</span>
            <span className="text-dim">from the README</span>
          </div>
          <dl className="divide-y">
            {COMMANDS.map(([when, cmd]) => (
              <div key={when} className="grid gap-1.5 py-3.5">
                <dt className="text-[13px] text-muted-foreground">{when}</dt>
                <dd>
                  <code className="block font-mono text-[12.5px] break-words text-foreground">
                    <span
                      aria-hidden="true"
                      className="mr-2 text-amber select-none"
                    >
                      $
                    </span>
                    {cmd}
                  </code>
                </dd>
              </div>
            ))}
          </dl>
        </div>
      </div>
    </section>
  )
}

function HowItWorks() {
  return (
    <section id="how" className="section scroll-mt-14">
      <div className="wrap">
        <SectionHead
          label="03 / How it works"
          title="From link to note in four steps."
        >
          tx fetches the text. Your own Claude or Codex does the analysis. tx
          shows you the result.
        </SectionHead>

        <ol className="relative mt-12 grid gap-0 sm:grid-cols-2 sm:gap-x-6 sm:gap-y-9 lg:grid-cols-4 lg:before:absolute lg:before:inset-x-1.5 lg:before:top-1.25 lg:before:h-px lg:before:bg-[linear-gradient(90deg,var(--border-strong),var(--amber-fill))] lg:before:content-['']">
          {STEPS.map((s, i) => (
            <li
              key={s.tag}
              className="relative pt-0 pb-7 pl-7.5 max-sm:before:absolute max-sm:before:top-2 max-sm:before:bottom-0 max-sm:before:left-[5px] max-sm:before:w-px max-sm:before:bg-border-strong sm:pt-7 sm:pb-0 sm:pl-0"
            >
              <span
                aria-hidden="true"
                className={
                  "absolute top-0.5 left-0 size-2.75 rounded-full max-sm:top-1.5 " +
                  (s.live
                    ? "bg-amber-fill shadow-[0_0_0_4px_color-mix(in_srgb,var(--amber-fill)_22%,transparent)]"
                    : "bg-background shadow-[inset_0_0_0_2px_var(--border-strong)]")
                }
              />
              <span className="mb-2.5 block font-mono text-[11px] text-amber">
                {String(i + 1).padStart(2, "0")} · {s.tag}
              </span>
              <h3 className="mb-2 text-[15px] font-semibold tracking-[-0.01em]">
                {s.title}
              </h3>
              <p className="text-[14.5px] leading-relaxed text-muted-foreground">
                {s.text}
              </p>
            </li>
          ))}
        </ol>

        <div className="mt-11 flex flex-wrap items-center justify-between gap-x-8 gap-y-4 border-t pt-6">
          <p className="max-w-[62ch] text-[15px]">
            <span className="eyebrow mb-1.5">What you need</span>
            Codex CLI or Claude Code, installed and signed in. You only need
            one. Fetching a transcript without AI needs neither.
          </p>
          <Button
            variant="outline"
            size="pill"
            nativeButton={false}
            render={<a href="#requirements" />}
            className="dark:bg-transparent"
          >
            Requirements
            <ArrowRight />
          </Button>
        </div>
      </div>
    </section>
  )
}

function QuickStart() {
  return (
    <section id="install" className="section scroll-mt-14 border-y bg-muted">
      <div className="wrap">
        <SectionHead
          label="04 / Quick start"
          title="Three commands to your first review."
        >
          Review one video first. Setup is optional for that.
        </SectionHead>

        <ol className="mt-11 grid border-t">
          <li className="grid gap-5 border-b py-7 md:grid-cols-[200px_1fr] md:gap-10">
            <div>
              <span className="font-mono text-[11px] text-amber">01</span>
              <h3 className="mt-2 text-[15px] font-semibold tracking-[-0.01em]">
                Install
              </h3>
            </div>
            <div className="max-w-[40rem] min-w-0">
              <InstallTabs />
              <ul className="mt-5 grid gap-2 text-[14px] text-muted-foreground">
                <li>
                  In a terminal, the installer starts guided setup. For a
                  one-off video review, skip setup with Ctrl+C. Run{" "}
                  <code className="font-mono text-foreground">tx onboard</code>{" "}
                  later to return to it.
                </li>
                <li>
                  If the installer added tx to your PATH, open a new terminal,
                  then check with{" "}
                  <code className="font-mono text-foreground">
                    tx --version
                  </code>
                  .
                </li>
                <li>
                  To stop the installer from opening setup, set{" "}
                  <code className="font-mono text-foreground">
                    TX_NO_SETUP=1
                  </code>{" "}
                  before installing.
                </li>
              </ul>
            </div>
          </li>

          <li className="grid gap-5 border-b py-7 md:grid-cols-[200px_1fr] md:gap-10">
            <div>
              <span className="font-mono text-[11px] text-amber">02</span>
              <h3 className="mt-2 text-[15px] font-semibold tracking-[-0.01em]">
                Try a video
              </h3>
            </div>
            <div className="max-w-[40rem] min-w-0">
              <CodeBlock code={`tx "${EXAMPLE_URL}" -q`} wrap />
              <p className="mt-3.5 text-[14px] text-muted-foreground">
                Replace the quoted URL with your own.{" "}
                <code className="font-mono text-foreground">-q</code> asks: “Is
                this video worth watching? Which parts should I watch, and why?”
                Add <code className="font-mono text-foreground">-g codex</code>{" "}
                to choose Codex.
              </p>
            </div>
          </li>

          <li className="grid gap-5 border-b py-7 md:grid-cols-[200px_1fr] md:gap-10">
            <div>
              <span className="font-mono text-[11px] text-amber">03</span>
              <h3 className="mt-2 text-[15px] font-semibold tracking-[-0.01em]">
                Save a note
              </h3>
            </div>
            <div className="max-w-[40rem] min-w-0">
              <CodeBlock
                code={`tx "${EXAMPLE_URL}" -g codex -a "3 timestamped takeaways as Markdown. Separate claims from evidence. Include the source URL." > video-notes.md`}
                wrap
              />
              <p className="mt-3.5 text-[14px] text-muted-foreground">
                <code className="font-mono text-foreground">&gt;</code> saves
                the answer in your current folder and replaces an existing file
                of that name. Review the note before you move it into your
                knowledge base. tx does not add it to a memory system for you.
              </p>
            </div>
          </li>
        </ol>
      </div>
    </section>
  )
}

function SourcesPrivacy() {
  return (
    <section className="section">
      <div className="wrap">
        <SectionHead
          label="05 / Sources and privacy"
          title="Honest about what works and what leaves your machine."
        >
          An upstream extractor is not a guarantee that every link works.
        </SectionHead>

        <div className="mt-11 grid gap-4 lg:grid-cols-2">
          <Card className="gap-0 bg-card py-0 shadow-none ring-border">
            <CardContent className="px-5 py-5">
              <div className="mb-1 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 border-b pb-3">
                <span className="eyebrow mb-0">Sources: more than YouTube</span>
                <span className="font-mono text-[11px] text-dim">
                  Checked with tx 0.18.1 on 2026-10-26
                </span>
              </div>
              <ul className="divide-y">
                {SOURCES.map(([name, what, ok]) => (
                  <li
                    key={name}
                    className="flex items-start justify-between gap-4 py-3.5"
                  >
                    <div className="min-w-0">
                      <p className="text-[14.5px] font-medium">{name}</p>
                      <p className="text-[13.5px] text-muted-foreground">
                        {what}
                      </p>
                    </div>
                    <Badge
                      variant="outline"
                      className={
                        "mt-0.5 font-mono " +
                        (ok
                          ? "border-ok/40 bg-ok/10 text-ok"
                          : "border-amber/40 bg-amber/10 text-amber")
                      }
                    >
                      {ok ? "checked" : "not verified"}
                    </Badge>
                  </li>
                ))}
              </ul>
              <p className="border-t pt-3.5 text-[13px] text-muted-foreground">
                Other upstream sources include Twitch, X, TikTok, Dailymotion,
                SoundCloud and Internet Archive. That is backend coverage, not
                additional end-to-end checks. Authentication, DRM, regional
                restrictions and site changes can stop extraction.
              </p>
            </CardContent>
          </Card>

          <Card className="gap-0 bg-card py-0 shadow-none ring-border">
            <CardContent className="px-5 py-5">
              <div className="mb-1 border-b pb-3">
                <span className="eyebrow mb-0">What stays on your device</span>
              </div>
              <ul className="divide-y">
                {[
                  [
                    "On your computer",
                    "Transcript fetching, caching and Whisper transcription. Files you save with > stay where you put them.",
                  ],
                  [
                    "Not necessarily local",
                    "AI analysis. Codex or Claude receives the transcript and may send it to its cloud service. Your provider's authentication, plan limits and usage terms apply.",
                  ],
                  [
                    "tx learn",
                    "Reads your coding-agent sessions on your computer and sends only redacted excerpts of your own prompts to the agent you choose. Nothing is read until you run it.",
                  ],
                  [
                    "Check the output",
                    "Generated takeaways can be wrong. Keep the source and timestamps so you can verify them.",
                  ],
                ].map(([k, v]) => (
                  <li key={k} className="py-3.5">
                    <p className="text-[14.5px] font-medium">{k}</p>
                    <p className="text-[13.5px] text-muted-foreground">{v}</p>
                  </li>
                ))}
              </ul>
            </CardContent>
          </Card>
        </div>
      </div>
    </section>
  )
}

function GoFurther() {
  const items = [
    {
      title: "Learn who you are",
      text: "tx reads your Claude Code, Codex and Oh My Pi sessions from the last 6 months on your computer, shows you which projects it found, and asks before sending anything. Reviews then name the projects a moment matters for.",
      cmd: "tx learn --dry-run",
      note: "Shows exactly what would be sent, without sending it.",
    },
    {
      title: "A brief every morning",
      text: "Try tx run by hand first. Then let your OS scheduler run it: a systemd user timer on Linux, a LaunchAgent on macOS, Task Scheduler on Windows. tx writes a Markdown digest and notifies you when something meets your score threshold.",
      cmd: "tx schedule --at 08:00",
      note: "Stop it with tx schedule --off.",
    },
    {
      title: "Inside your AI app",
      text: "tx mcp is a standard stdio MCP server. Connect Codex, Claude Code or Claude Desktop, restart the app, then ask “Show me my tx brief.” You do not need this to review a link.",
      cmd: "tx setup codex",
      note: "Also: tx setup claude-code, tx setup claude-desktop.",
    },
  ]
  return (
    <section className="section border-y bg-muted">
      <div className="wrap">
        <p className="eyebrow">06 / Go further</p>
        <h2 className="h2">When one video turns into a habit.</h2>
        <div className="mt-11 grid gap-10 md:grid-cols-3">
          {items.map((it) => (
            <div key={it.title} className="flex flex-col border-t pt-4.5">
              <h3 className="mb-1.5 text-[15px] font-semibold tracking-[-0.01em]">
                {it.title}
              </h3>
              <p className="mb-5 flex-1 text-[14.5px] text-muted-foreground">
                {it.text}
              </p>
              <CodeBlock code={it.cmd} wrap />
              <p className="mt-2.5 min-h-8 font-mono text-[11px] text-dim">
                {it.note}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

function Requirements() {
  return (
    <section id="requirements" className="section scroll-mt-14">
      <div className="wrap">
        <SectionHead
          label="07 / Requirements"
          title="What you need, and what it is."
        >
          No tx account. No separate tx subscription or API key.
        </SectionHead>

        <div className="mt-11 grid gap-4 lg:grid-cols-[1.15fr_1fr_0.85fr]">
          <Card className="gap-0 bg-card py-0 shadow-none ring-amber/50 lg:shadow-(--shadow-card)">
            <CardContent className="flex h-full flex-col gap-1.5 px-5.5 py-5.5">
              <div className="mb-3 flex items-center justify-between">
                <span className="eyebrow mb-0">To run tx</span>
                <Badge
                  className="border-amber/40 bg-amber/10 font-mono text-amber"
                  variant="outline"
                >
                  one is enough
                </Badge>
              </div>
              <ul className="grid gap-2.5 text-[14px]">
                <li className="relative pl-6.5 before:absolute before:top-[6px] before:left-0 before:size-3.5 before:rounded-sm before:border before:border-border-strong before:bg-[linear-gradient(var(--amber-fill),var(--amber-fill))] before:bg-size-[6px_6px] before:bg-center before:bg-no-repeat">
                  <a
                    className="underline underline-offset-4"
                    href="https://learn.chatgpt.com/docs/codex/cli"
                  >
                    Codex CLI
                  </a>{" "}
                  or{" "}
                  <a
                    className="underline underline-offset-4"
                    href="https://code.claude.com/docs/en/quickstart"
                  >
                    Claude Code
                  </a>
                  , installed and signed in.
                </li>
                <li className="relative pl-6.5 before:absolute before:top-[6px] before:left-0 before:size-3.5 before:rounded-sm before:border before:border-border-strong before:bg-[linear-gradient(var(--amber-fill),var(--amber-fill))] before:bg-size-[6px_6px] before:bg-center before:bg-no-repeat">
                  macOS (Apple Silicon), Linux (x86_64) or Windows (x64).
                </li>
                <li className="relative pl-6.5 before:absolute before:top-[6px] before:left-0 before:size-3.5 before:rounded-sm before:border before:border-border-strong before:bg-[linear-gradient(var(--amber-fill),var(--amber-fill))] before:bg-size-[6px_6px] before:bg-center before:bg-no-repeat">
                  Each reviewed video is a separate request through your Claude
                  or Codex account. Plan limits apply.
                </li>
              </ul>
              <Button
                size="pill"
                nativeButton={false}
                render={<a href="#install" />}
                className="mt-auto w-full"
              >
                Install tx
              </Button>
            </CardContent>
          </Card>

          <Card className="gap-0 bg-card py-0 shadow-none ring-border">
            <CardContent className="flex h-full flex-col gap-1.5 px-5.5 py-5.5">
              <span className="eyebrow mb-3">Open source</span>
              <p className="text-[14px] text-muted-foreground">
                MIT licensed. Latest release: v{VERSION}. To build from source
                you need Rust, CMake and Clang.
              </p>
              <div className="mt-3 mb-4">
                <CodeBlock
                  code="git clone https://github.com/jrydel/tx && cd tx && cargo build --release -p tx-cli"
                  wrap
                />
              </div>
              <div className="mt-auto flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  size="pill-sm"
                  nativeButton={false}
                  render={<a href={REPO_URL} />}
                  className="dark:bg-transparent"
                >
                  <GithubIcon />
                  GitHub
                </Button>
                <Button
                  variant="outline"
                  size="pill-sm"
                  nativeButton={false}
                  render={<a href={RELEASES_URL} />}
                  className="dark:bg-transparent"
                >
                  Releases
                </Button>
                <Button
                  variant="outline"
                  size="pill-sm"
                  nativeButton={false}
                  render={<a href={LICENSE_URL} />}
                  className="dark:bg-transparent"
                >
                  License
                </Button>
              </div>
            </CardContent>
          </Card>

          <Card className="gap-0 bg-transparent py-0 shadow-none ring-border-strong">
            <CardContent className="flex h-full flex-col gap-1.5 px-5.5 py-5.5">
              <span className="eyebrow mb-3">Tutorial</span>
              <p className="mb-4 flex-1 text-[14px] text-muted-foreground">
                74 seconds, one continuous terminal session: install, finish
                setup, then try the use cases. Silent, with captions.
              </p>
              <a
                href={TUTORIAL_URL}
                className="inline-flex items-center gap-1 text-[13.5px] font-medium hover:text-amber"
              >
                Download the MP4
                <ArrowUpRight className="size-3.5" />
              </a>
            </CardContent>
          </Card>
        </div>
      </div>
    </section>
  )
}

function FinalCta() {
  return (
    <section className="section border-t bg-muted text-center">
      <div className="wrap">
        <h2 className="h2 mx-auto">Try it on one video.</h2>
        <p className="mx-auto mt-2 max-w-[32em] text-[16.5px] text-balance text-muted-foreground">
          Start small: review one video first. If the result is useful, add a
          profile.
        </p>
        <div className="mt-7 flex flex-wrap justify-center gap-2.5">
          <Button
            size="pill"
            nativeButton={false}
            render={<a href="#install" />}
          >
            Install tx
          </Button>
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
    </section>
  )
}
