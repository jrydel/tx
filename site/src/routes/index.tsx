import * as React from "react"
import { createFileRoute } from "@tanstack/react-router"
import { InstallTabs } from "@/components/install-tabs"
import { SiteFooter } from "@/components/site-footer"
import { SiteHeader } from "@/components/site-header"
import { ReviewTerminal } from "@/components/terminal"
import { VERSION } from "@/lib/site"

export const Route = createFileRoute("/")({ component: Home })

/** Source of every claim: README.md, install.sh, install.ps1, .github/assets/tx-tutorial.jpg. */
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
        className="wrap flex flex-col gap-14 pt-14 pb-20 sm:pt-20"
      >
        <section className="flex flex-col items-start gap-5">
          <p className="rise font-mono text-[11px] tracking-normal text-muted-foreground uppercase">
            v{VERSION} · Open source
          </p>
          <h1
            className="rise font-heading text-[clamp(30px,4.2vw,44px)] leading-[1.05] font-bold tracking-[-0.035em]"
            style={d(1)}
          >
            Is this video worth watching?
          </h1>
          <p
            className="rise max-w-[34em] text-[15px] leading-relaxed text-muted-foreground"
            style={d(2)}
          >
            Paste a video or podcast link. Your own Claude or Codex answers{" "}
            <b className="font-semibold text-ok">WATCH</b>,{" "}
            <b className="font-semibold text-amber">SKIM</b> or{" "}
            <b className="font-semibold text-coral">SKIP</b>, with timestamped
            takeaways.
          </p>
          <div
            className="rise flex w-full flex-col gap-3 sm:flex-row sm:items-start"
            style={d(3)}
          >
            <div className="min-w-0 flex-1">
              <InstallTabs />
            </div>
          </div>
        </section>

        <section aria-label="Example output" className="rise" style={d(4)}>
          <ReviewTerminal className="text-[12px] leading-[1.65] sm:text-[12.5px] xl:text-[13px]" />
          <p className="mt-3 text-center font-mono text-[10.5px] text-dim">
            Real tx 0.20.0 output, example profile.
          </p>
        </section>

        <section aria-labelledby="commands" className="flex flex-col gap-4">
          <h2
            id="commands"
            className="font-mono text-[11px] tracking-normal text-muted-foreground uppercase"
          >
            Commands
          </h2>
          <Commands />
          <p className="text-[13px] leading-relaxed text-muted-foreground">
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
            , signed in. Runs on macOS, Linux and Windows, on your computer: no
            tx account.
          </p>
        </section>
      </main>
      <SiteFooter />
    </>
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
      className="shrink-0 rounded-md px-1.5 text-[10.5px] text-(--t-grey) opacity-0 transition-opacity group-hover:opacity-100 hover:text-(--t-fg) focus-visible:opacity-100"
      aria-label={`Copy ${text}`}
    >
      {copied ? "Copied" : "Copy"}
    </button>
  )
}

function Commands() {
  return (
    <figure className="term overflow-hidden rounded-xl font-mono text-[12px] leading-relaxed shadow-(--shadow-term) sm:text-[12.5px]">
      <ul className="flex flex-col gap-3.5 px-5 py-5 sm:px-6">
        {COMMANDS.map(([label, cmd]) => (
          <li key={label} className="group">
            <p className="text-(--t-grey)"># {label}</p>
            <div className="flex items-start justify-between gap-3">
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
  )
}
