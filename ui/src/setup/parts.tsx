import type { ReactNode } from "react"
import { cn } from "@/lib/utils"

export function H2({ children }: { children: ReactNode }) {
  return <h2 className="m-0 mb-0.5 text-base font-bold tracking-[-.01em] group-data-compact/setup:mb-2">{children}</h2>
}

/** A line of explanation. `lead` ones are the first thing dropped in the compact layout. */
export function Hint({ children, lead, className }: { children: ReactNode; lead?: boolean; className?: string }) {
  return (
    <p className={cn("m-0 mb-2.5 text-[13.5px] text-muted-foreground", lead && "group-data-compact/setup:hidden", className)}>
      {children}
    </p>
  )
}
