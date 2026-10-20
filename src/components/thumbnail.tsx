import { timecode } from "@/lib/time"
import { cn } from "@/lib/utils"

/** YouTube's frame for the video, with its runtime on a film-dark slate chip. */
export function Thumbnail({
  id,
  duration,
  className,
}: {
  id: string
  duration: number | null
  className?: string
}) {
  return (
    <div
      className={cn(
        "relative aspect-video overflow-hidden rounded-md bg-strip",
        className
      )}
    >
      <img
        src={`https://i.ytimg.com/vi/${id}/mqdefault.jpg`}
        alt=""
        loading="lazy"
        className="size-full object-cover"
        onError={(e) => {
          e.currentTarget.style.visibility = "hidden"
        }}
      />
      {duration !== null && (
        <span className="timecode absolute right-1.5 bottom-1.5 rounded-[3px] bg-strip/90 px-1.5 py-0.5 text-[10px] text-white">
          {timecode(duration)}
        </span>
      )}
    </div>
  )
}
