import { Badge } from "@/components/ui/badge"

/** Relevance 0–5 against the user's profile. 4–5 get the grease-pencil mark. */
export function ScoreBadge({ relevance }: { relevance: number | null }) {
  if (relevance === null) return null
  const variant =
    relevance >= 4 ? "marker" : relevance === 3 ? "marker-outline" : "secondary"
  return (
    <Badge variant={variant} aria-label={`Relevance ${relevance} of 5`}>
      <span className="timecode">{relevance}/5</span>
    </Badge>
  )
}
