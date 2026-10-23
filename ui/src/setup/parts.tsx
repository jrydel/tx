import type { ReactNode } from "react"
import { CheckIcon } from "lucide-react"
import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { cn } from "@/lib/utils"

/**
 * One question of setup in its own card. Open: the question, what it's for, then the step's own CardContent and
 * CardFooter (its actions). Closed: one line with the question and what was answered; clicking it opens it.
 */
export function StepBox({
  n,
  title,
  description,
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
  /** What the answer is used for: under the question while it's open. */
  description?: string
  open: boolean
  done: boolean
  summary?: string
  onOpen?: () => void
  /** When open, take the panel's spare height (a long list inside grows with it; the cards below stay in view). */
  grow?: boolean
  children?: ReactNode
}) {
  const mark = (
    <span
      aria-hidden
      className={cn(
        "grid size-6 shrink-0 place-items-center rounded-full text-[12px] font-semibold tabular-nums",
        done ? "bg-watch text-card" : open ? "bg-foreground text-card" : "ring-1 ring-foreground/20 text-faint"
      )}
    >
      {done ? <CheckIcon className="size-3.5" strokeWidth={3} /> : n}
    </span>
  )
  if (!open) {
    return (
      <button
        type="button"
        onClick={onOpen}
        title={done ? `${title} Change it` : title}
        className="flex w-full cursor-pointer items-center gap-3 rounded-xl bg-card px-5 py-3.5 text-left ring-1 ring-foreground/10 transition-shadow hover:ring-foreground/25 group-data-compact/setup:px-4 group-data-compact/setup:py-2.5"
      >
        {mark}
        <span className={cn("shrink-0 text-[15px] font-semibold", !done && "text-muted-foreground")}>{title}</span>
        <span className="min-w-0 flex-1 truncate text-right text-[13px] text-faint">{summary}</span>
        {done && <span className="shrink-0 text-[13px] font-medium text-muted-foreground">Edit</span>}
      </button>
    )
  }
  return (
    <Card
      aria-label={title}
      className={cn(
        "min-h-0 gap-5 ring-foreground/20 [--card-spacing:--spacing(5)] *:shrink-0",
        "group-data-compact/setup:gap-3 group-data-compact/setup:[--card-spacing:--spacing(4)]",
        grow && "flex-1 !shrink"
      )}
    >
      <CardHeader>
        <CardTitle className="flex items-center gap-3 text-[17px] font-semibold tracking-[-.01em]">
          {mark}
          {title}
        </CardTitle>
        {description && <CardDescription className="pl-9 text-[13.5px] group-data-compact/setup:hidden">{description}</CardDescription>}
      </CardHeader>
      {children}
    </Card>
  )
}

/** A line of explanation. */
export function Hint({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <p className={cn("m-0 mb-2.5 text-[13.5px] text-muted-foreground", className)}>
      {children}
    </p>
  )
}
