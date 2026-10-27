import { Wordmark } from "@/components/logo"
import {
  AUTHOR_URL,
  LICENSE_URL,
  RELEASES_URL,
  REPO_URL,
  STUDIO_URL,
} from "@/lib/site"

const LINKS = [
  { href: REPO_URL, label: "GitHub" },
  { href: RELEASES_URL, label: "Releases" },
  { href: LICENSE_URL, label: "License" },
]

export function SiteFooter() {
  const link =
    "underline decoration-border-strong underline-offset-4 transition-colors hover:text-foreground hover:decoration-foreground"
  return (
    <footer className="border-t pt-7 pb-10">
      <div className="wrap flex flex-wrap items-center justify-between gap-x-7 gap-y-4">
        <Wordmark className="text-[15px]" />
        <nav
          aria-label="Footer"
          className="flex flex-wrap gap-x-4.5 gap-y-2 text-[13px] text-muted-foreground"
        >
          {LINKS.map((l) => (
            <a
              key={l.href}
              href={l.href}
              className="transition-colors hover:text-foreground"
            >
              {l.label}
            </a>
          ))}
        </nav>
        <p className="w-full font-mono text-[11px] text-dim">
          tx is open source (
          <a href={LICENSE_URL} className={link}>
            MIT
          </a>
          ) · built by{" "}
          <a href={AUTHOR_URL} className={link}>
            Jiří Rýdel
          </a>{" "}
          · a{" "}
          <a href={STUDIO_URL} className={link}>
            TodayWorks
          </a>{" "}
          tool
        </p>
      </div>
    </footer>
  )
}
