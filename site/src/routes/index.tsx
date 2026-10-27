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
      <div className="wrap grid items-center gap-14 lg:grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)] lg:gap-20 2xl:gap-28">
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

          <div className="rise mt-9 max-w-[36rem]" style={d(3)}>
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

function HowItWorks() {
  return (
    <section id="how" className="border-t py-16 sm:py-20">
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
    <section id="requirements" className="border-t py-16 sm:py-20">
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
