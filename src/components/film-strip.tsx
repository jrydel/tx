import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import type { Takeaway, WatchAdvice } from "@/lib/analysis"
import { timecode } from "@/lib/time"
import { cn } from "@/lib/utils"

type Props = {
  duration: number
  takeaways: Takeaway[]
  watch: WatchAdvice
  onSeek: (seconds: number) => void
  size?: "sm" | "lg"
  className?: string
}

/**
 * The whole video as one strip of film. Grease-pencil ticks mark where each takeaway is said;
 * a marked band is the stretch the analysis says to skim. Ticks are real buttons: they jump there.
 */
export function FilmStrip({
  duration,
  takeaways,
  watch,
  onSeek,
  size = "sm",
  className,
}: Props) {
  const pct = (s: number) =>
    `${Math.min(100, Math.max(0, (s / duration) * 100))}%`
  const band =
    watch.kind === "skim" &&
    watch.from !== null &&
    watch.to !== null &&
    watch.to > watch.from
      ? {
          left: pct(watch.from),
          width: `calc(${pct(watch.to)} - ${pct(watch.from)})`,
        }
      : watch.kind === "watch"
        ? { left: "0%", width: "100%" }
        : null
  const marks = takeaways.filter(
    (t): t is Takeaway & { at: number } => t.at !== null && t.at <= duration
  )

  return (
    <div className={cn("flex flex-col gap-1.5", className)}>
      <div
        className={cn(
          "filmstrip w-full",
          size === "lg" ? "h-10" : "h-[22px]",
          watch.kind === "skip" && "opacity-60"
        )}
        role="group"
        aria-label={`Timeline, ${timecode(duration)}`}
      >
        {band && (
          <div
            className="absolute inset-y-[7px] rounded-[1px] bg-marker/35 shadow-[inset_0_0_0_1px_var(--marker)]"
            style={band}
            aria-hidden
          />
        )}
        {marks.map((t, i) => (
          <Tooltip key={`${t.at}-${i}`}>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={(e) => {
                  e.preventDefault()
                  e.stopPropagation()
                  onSeek(t.at)
                }}
                className="group/tick absolute inset-y-0 w-4 -translate-x-1/2 outline-none"
                style={{ left: pct(t.at) }}
                aria-label={`Jump to ${timecode(t.at)}: ${t.text}`}
              >
                <span className="absolute inset-y-[3px] left-1/2 w-[3px] -translate-x-1/2 rounded-full bg-marker transition-transform group-hover/tick:scale-x-150 group-focus-visible/tick:scale-x-150 group-focus-visible/tick:ring-2 group-focus-visible/tick:ring-ring motion-reduce:transition-none" />
              </button>
            </TooltipTrigger>
            <TooltipContent className="max-w-xs">
              <span className="timecode mr-1.5 text-marker">
                {timecode(t.at)}
              </span>
              {t.text}
            </TooltipContent>
          </Tooltip>
        ))}
      </div>
      {size === "lg" && (
        <div className="timecode flex justify-between text-[11px] text-muted-foreground">
          <span>00:00</span>
          {band &&
            watch.kind === "skim" &&
            watch.from !== null &&
            watch.to !== null && (
              <span>
                skim {timecode(watch.from)}–{timecode(watch.to)}
              </span>
            )}
          <span>{timecode(duration)}</span>
        </div>
      )}
    </div>
  )
}
