import type { ReactNode } from "react"
import { CheckIcon } from "lucide-react"
import { cn } from "@/lib/utils"

/**
 * One question of setup in its own box. Open: the question and its answer form. Closed: the question, and what was
 * answered (or nothing yet); clicking it opens it.
 */
export function StepBox({
  n,
  title,
  open,
  done,
  summary,
  onOpen,
  grow,
  children,
}: {
  /** Its place in the order, shown until it's answered. */
  n?: number
  title: string
  open: boolean
  done: boolean
  summary?: string
  onOpen?: () => void
  /** When open, take the panel's spare height (a long list inside shrinks to fit, so the boxes below stay in view). */
  grow?: boolean
  children: ReactNode
}) {
  const mark = (
    <span
      aria-hidden
      className={cn(
        "grid size-5 shrink-0 place-items-center rounded-full text-[11px] font-bold",
        done ? "bg-watch text-card" : open ? "bg-foreground text-card" : "border border-border text-faint"
      )}
    >
      {done ? <CheckIcon className="size-3" strokeWidth={3} /> : n}
    </span>
  )
  if (!open) {
    return (
      <button
        type="button"
        onClick={onOpen}
        title={done ? `${title} Change it` : title}
        className="flex w-full cursor-pointer items-center gap-2.5 rounded-xl border bg-card px-3.5 py-2.5 text-left hover:border-faint"
      >
        {mark}
        <span className={cn("shrink-0 text-[14px] font-semibold", !done && "text-muted-foreground")}>{title}</span>
        <span className="min-w-0 flex-1 truncate text-right text-[13px] text-faint">{summary}</span>
        {done && <span className="shrink-0 text-[12.5px] font-semibold text-muted-foreground">Edit</span>}
      </button>
    )
  }
  return (
    <section
      aria-label={title}
      className={cn(
        "flex min-h-0 flex-col rounded-xl border border-faint/60 bg-card px-3.5 py-3 *:shrink-0 group-data-compact/setup:py-2.5",
        grow && "flex-1 !shrink"
      )}
    >
      <h2 className="m-0 mb-1.5 flex items-center gap-2.5 text-[15px] font-bold tracking-[-.01em]">
        {mark}
        {title}
      </h2>
      {children}
    </section>
  )
}

/** A line of explanation. `lead` ones are the first thing dropped in the compact layout. */
export function Hint({ children, lead, className }: { children: ReactNode; lead?: boolean; className?: string }) {
  return (
    <p className={cn("m-0 mb-2.5 text-[13.5px] text-muted-foreground", lead && "group-data-compact/setup:hidden", className)}>
      {children}
    </p>
  )
}
