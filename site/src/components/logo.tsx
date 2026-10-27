import { cn } from "@/lib/utils"

/** The tx mark: a lowercase "tx" in a terminal-dark tile. Matches public/favicon.svg. */
export function LogoMark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 32 32"
      aria-hidden="true"
      className={cn("size-6", className)}
    >
      <rect width="32" height="32" rx="7" fill="#171b1c" />
      <g
        fill="none"
        strokeWidth="2.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M11 7.5v11.2c0 1.8.9 2.8 2.7 2.8" stroke="#e8ecec" />
        <path d="M7 12.5h8" stroke="#e8ecec" />
        <path d="m18.5 12.5 7 9M25.5 12.5l-7 9" stroke="#f6b83d" />
      </g>
    </svg>
  )
}

/** Typographic wordmark: mono "tx" with a blinking amber cursor. */
export function Wordmark({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-2 font-mono text-xl font-bold tracking-tight",
        className
      )}
    >
      <span className="inline-flex items-baseline">
        tx
        <span
          aria-hidden="true"
          className="cursor ml-0.5 inline-block h-[0.95em] w-[0.5em] translate-y-[0.12em] bg-amber-fill"
        />
      </span>
    </span>
  )
}
