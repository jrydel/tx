import { Moon, Sun } from "lucide-react"
import { Button } from "@/components/ui/button"

export const THEME_KEY = "tx-theme"

/** Runs before first paint: stored choice, else the OS preference, dark by default. */
export const THEME_SCRIPT = `(function(){try{var t=localStorage.getItem("${THEME_KEY}");if(!t)t=matchMedia("(prefers-color-scheme: light)").matches?"light":"dark";document.documentElement.classList.toggle("dark",t==="dark");document.documentElement.style.colorScheme=t}catch(e){}})()`

function toggleTheme() {
  const root = document.documentElement
  const next = root.classList.contains("dark") ? "light" : "dark"
  root.classList.toggle("dark", next === "dark")
  root.style.colorScheme = next
  try {
    localStorage.setItem(THEME_KEY, next)
  } catch {
    // Storage can be blocked; the toggle still works for this visit.
  }
}

export function ThemeToggle() {
  return (
    <Button
      variant="ghost"
      size="icon"
      aria-label="Toggle colour theme"
      onClick={toggleTheme}
      className="rounded-lg text-muted-foreground hover:text-foreground"
    >
      <Sun className="hidden dark:block" />
      <Moon className="dark:hidden" />
    </Button>
  )
}
