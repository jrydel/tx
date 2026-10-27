import { useEffect, useRef, useState } from "react"
import { Check, Copy } from "lucide-react"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

async function writeClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    // Insecure context or denied permission: fall back to a hidden textarea.
    const area = document.createElement("textarea")
    area.value = text
    area.setAttribute("readonly", "")
    area.style.position = "fixed"
    area.style.opacity = "0"
    document.body.appendChild(area)
    area.select()
    const ok = document.execCommand("copy")
    area.remove()
    return ok
  }
}

export function CopyButton({
  text,
  label = "Copy command",
  className,
}: {
  text: string
  label?: string
  className?: string
}) {
  const [copied, setCopied] = useState(false)
  const timer = useRef<number | undefined>(undefined)

  useEffect(() => () => window.clearTimeout(timer.current), [])

  async function copy() {
    if (!(await writeClipboard(text))) return
    setCopied(true)
    window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => setCopied(false), 1800)
  }

  return (
    <Button
      variant="ghost"
      size="icon-sm"
      onClick={copy}
      aria-label={copied ? "Copied" : label}
      className={cn(
        "shrink-0 text-muted-foreground hover:text-foreground",
        copied && "text-ok hover:text-ok",
        className
      )}
    >
      {copied ? <Check /> : <Copy />}
      <span role="status" className="sr-only">
        {copied ? "Copied to clipboard" : ""}
      </span>
    </Button>
  )
}

/** A command in a bordered mono box with a copy button. */
export function CodeBlock({
  code,
  prompt = "$",
  wrap = false,
  className,
}: {
  code: string
  prompt?: string
  wrap?: boolean
  className?: string
}) {
  return (
    <div
      className={cn(
        "flex items-start gap-2 rounded-xl border border-border-strong bg-background py-1.5 pr-1.5 pl-3.5 font-mono text-[13px]",
        className
      )}
    >
      <span aria-hidden="true" className="py-1 text-cyan select-none">
        {prompt}
      </span>
      <code
        className={cn(
          "min-w-0 flex-1 py-1 text-foreground",
          wrap
            ? "break-words whitespace-pre-wrap"
            : "overflow-x-auto whitespace-pre"
        )}
        tabIndex={wrap ? undefined : 0}
      >
        {code}
      </code>
      <CopyButton text={code} />
    </div>
  )
}
