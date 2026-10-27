import { Button } from "@/components/ui/button"
import { GithubIcon } from "@/components/icons"
import { Wordmark } from "@/components/logo"
import { ThemeToggle } from "@/components/theme-toggle"
import { REPO_URL } from "@/lib/site"

const LINKS = [
  { href: "#how", label: "How it works" },
  { href: "#install", label: "Install" },
  { href: "#requirements", label: "Requirements" },
]

export function SiteHeader() {
  return (
    <header className="sticky top-0 z-20 border-b bg-background/80 backdrop-blur-md backdrop-saturate-150">
      <div className="wrap flex h-14.5 items-center justify-between">
        <a href="/" aria-label="tx home">
          <Wordmark />
        </a>
        <nav aria-label="Main" className="flex items-center gap-1.5 sm:gap-3.5">
          {LINKS.map((l) => (
            <a
              key={l.href}
              href={l.href}
              className="hidden text-[13.5px] text-muted-foreground transition-colors hover:text-foreground md:inline"
            >
              {l.label}
            </a>
          ))}
          <ThemeToggle />
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
            size="pill-sm"
            nativeButton={false}
            render={<a href="#install" />}
            className="hidden sm:inline-flex"
          >
            Install
          </Button>
        </nav>
      </div>
    </header>
  )
}
