import { AUTHOR_URL, LICENSE_URL, STUDIO_URL } from "@/lib/site"

export function SiteFooter() {
  const link =
    "underline decoration-border-strong underline-offset-4 transition-colors hover:text-foreground hover:decoration-foreground"
  return (
    <footer className="border-t py-10">
      <div className="wrap">
        <p className="font-mono text-[12px] text-muted-foreground">
          <a href={LICENSE_URL} className={link}>
            MIT
          </a>{" "}
          · built by{" "}
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
