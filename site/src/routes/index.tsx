import * as React from "react"
import { createFileRoute } from "@tanstack/react-router"
import { Check, Copy } from "lucide-react"
import {
  AUTHOR_URL,
  INSTALL,
  LICENSE_URL,
  REPO_URL,
  STUDIO_URL,
  VERSION,
} from "@/lib/site"
import { cn } from "@/lib/utils"

export const Route = createFileRoute("/")({ component: Home })

type Os = keyof typeof INSTALL

const OS_LABEL: Record<Os, string> = {
  unix: "macOS · Linux",
  windows: "Windows",
}

function Home() {
  const d = (n: number) => ({ "--d": n }) as React.CSSProperties
  return (
    <div className="one-screen flex min-h-dvh flex-col">
      <header className="flex h-12 flex-none items-center justify-between px-[max(22px,calc((100vw-1024px)/2))] text-[13px]">
        <a
          href="/"
          aria-label="tx home"
          className="text-[19px] font-bold tracking-[-0.04em]"
        >
          tx
        </a>
        <a
          href={REPO_URL}
          className="text-muted-foreground transition-colors hover:text-foreground"
        >
          GitHub
        </a>
      </header>

      <main className="flex min-h-0 flex-1 flex-col items-center justify-start px-[22px] pt-6 pb-9 text-center min-[721px]:justify-center min-[721px]:pt-0 min-[721px]:pb-0">
        <h1
          className="rise font-heading text-[clamp(42px,5.6vw,82px)] leading-[1.04] font-semibold tracking-[-0.034em] text-balance"
          style={d(0)}
        >
          Is this video worth watching?
        </h1>
        <p
          className="rise mt-3.5 text-[clamp(18px,1.55vw,22px)] leading-[1.3] tracking-[-0.012em] text-balance text-muted-foreground"
          style={d(1)}
        >
          Paste a link. Your own Claude or Codex says{" "}
          <b className="font-medium text-foreground">watch</b>,{" "}
          <b className="font-medium text-foreground">skim</b> or{" "}
          <b className="font-medium text-foreground">skip</b>.
        </p>

        <div className="rise rise-mac mac mt-[30px] flex-none" style={d(2)}>
          <div className="mac-lid">
            <div className="mac-bezel">
              <div className="mac-screen">
                <TutorialVideo />
              </div>
            </div>
          </div>
          <div className="mac-base" aria-hidden="true" />
        </div>

        <div
          className="rise mt-[30px] flex w-full flex-none flex-col items-center gap-3 min-[721px]:mt-[38px] min-[721px]:w-auto"
          style={d(4)}
        >
          <InstallPill />
          <p className="max-w-[28ch] text-[13px] tracking-[-0.005em] text-balance text-faint min-[721px]:max-w-none">
            Runs on your computer. Needs Claude Code or Codex CLI, signed in.
          </p>
        </div>
      </main>

      <footer className="flex-none px-[22px] pb-7 text-center text-xs leading-normal text-faint min-[721px]:pb-5">
        <ul className="flex flex-col items-center min-[721px]:flex-row min-[721px]:justify-center min-[721px]:[&>li+li]:before:mx-[9px] min-[721px]:[&>li+li]:before:content-['·']">
          <li>
            Open source ·{" "}
            <a
              href={LICENSE_URL}
              className="underline decoration-black/25 underline-offset-2 hover:text-foreground"
            >
              MIT
            </a>
          </li>
          <li>Version {VERSION}</li>
          <li>
            Built by{" "}
            <a
              href={AUTHOR_URL}
              className="underline decoration-black/25 underline-offset-2 hover:text-foreground"
            >
              Jiří Rýdel
            </a>
          </li>
          <li>
            A{" "}
            <a
              href={STUDIO_URL}
              className="underline decoration-black/25 underline-offset-2 hover:text-foreground"
            >
              TodayWorks
            </a>{" "}
            tool
          </li>
        </ul>
      </footer>
    </div>
  )
}

/** The README tutorial (release asset tx-terminal.mp4, re-encoded to 720p; silent, with captions). */
function TutorialVideo() {
  const ref = React.useRef<HTMLVideoElement>(null)
  React.useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return
    void ref.current?.play().catch(() => undefined)
  }, [])
  return (
    <video
      ref={ref}
      className="block size-full bg-black object-cover"
      src="/tx-tutorial.mp4"
      poster="/tx-tutorial-poster.jpg"
      width={1280}
      height={720}
      muted
      loop
      playsInline
      controls
      preload="metadata"
      aria-label="tx tutorial, 74 seconds: install, setup, then reviewing videos"
    />
  )
}

function InstallPill() {
  const [os, setOs] = React.useState<Os>("unix")
  const [copied, setCopied] = React.useState(false)
  const timer = React.useRef<number>(undefined)

  const copy = async () => {
    const text = INSTALL[os]
    try {
      await navigator.clipboard.writeText(text)
    } catch {
      // Clipboard API blocked (insecure context, permissions): fall back to a hidden textarea.
      const area = document.createElement("textarea")
      area.value = text
      area.style.position = "fixed"
      area.style.opacity = "0"
      document.body.append(area)
      area.select()
      document.execCommand("copy")
      area.remove()
    }
    setCopied(true)
    window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => setCopied(false), 1800)
  }

  return (
    <div className="flex w-full max-w-full flex-col items-stretch gap-2.5 rounded-[26px] border bg-muted p-2 min-[721px]:w-auto min-[721px]:flex-row min-[721px]:items-center min-[721px]:gap-3.5 min-[721px]:rounded-full min-[721px]:p-1.5">
      <div
        role="group"
        aria-label="Operating system"
        className="flex flex-none rounded-full bg-black/[0.055] p-[3px]"
      >
        {(Object.keys(INSTALL) as Array<Os>).map((key) => (
          <button
            key={key}
            type="button"
            aria-pressed={os === key}
            onClick={() => {
              setOs(key)
              setCopied(false)
            }}
            className={cn(
              "flex-1 cursor-pointer rounded-full px-3.5 py-[9px] text-[13px] leading-none font-medium tracking-[-0.005em] whitespace-nowrap transition-[background,color,box-shadow] duration-200 max-[720px]:py-[11px]",
              os === key
                ? "bg-white text-foreground shadow-[0_1px_2px_rgb(0_0_0/0.14),0_0_0_0.5px_rgb(0_0_0/0.06)]"
                : "text-muted-foreground hover:text-foreground"
            )}
          >
            {OS_LABEL[key]}
          </button>
        ))}
      </div>
      <code className="min-w-0 [scrollbar-width:none] overflow-x-auto px-2 py-1 text-left font-mono text-[11.5px] leading-[1.55] tracking-[-0.01em] break-all min-[721px]:px-0 min-[721px]:text-[12.5px] min-[721px]:leading-[1.2] min-[721px]:break-normal min-[721px]:whitespace-nowrap">
        {INSTALL[os]}
      </code>
      <button
        type="button"
        onClick={() => void copy()}
        aria-label="Copy install command"
        className={cn(
          "inline-flex h-11 flex-none cursor-pointer items-center justify-center gap-[7px] rounded-full pr-[18px] pl-[15px] text-sm leading-none font-medium tracking-[-0.01em] text-white transition-[background,transform] duration-150 active:scale-[0.97] min-[721px]:h-[38px]",
          copied ? "bg-foreground" : "bg-blue hover:bg-blue-hover"
        )}
      >
        {copied ? (
          <Check aria-hidden="true" className="size-[15px]" />
        ) : (
          <Copy aria-hidden="true" className="size-[15px]" />
        )}
        <span aria-live="polite">{copied ? "Copied" : "Copy"}</span>
      </button>
    </div>
  )
}
