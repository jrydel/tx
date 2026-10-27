import { Button } from "@/components/ui/button"
import { GithubIcon } from "@/components/icons"
import { Wordmark } from "@/components/logo"
import { ThemeToggle } from "@/components/theme-toggle"
import { REPO_URL } from "@/lib/site"

export function SiteHeader() {
  return (
    <header className="sticky top-0 z-20 border-b bg-background/80 backdrop-blur-md backdrop-saturate-150">
      <div className="wrap flex h-14.5 items-center justify-between">
        <a href="/" aria-label="tx home">
          <Wordmark />
        </a>
        <nav aria-label="Main" className="flex items-center gap-2">
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
        </nav>
      </div>
    </header>
  )
}
