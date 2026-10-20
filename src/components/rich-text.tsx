import { Fragment } from "react"

// **bold**, `code`, and [mm:ss] timestamps. Rendered as React nodes, so text from analyses and
// transcripts (untrusted: video creators control it) can never become markup.
const TOKEN = /(\*\*.+?\*\*|`[^`]+`|\[(?:(\d+):)?(\d{1,2}):(\d{2})\])/g

export function RichText({
  text,
  onSeek,
}: {
  text: string
  onSeek?: (seconds: number) => void
}) {
  const parts: React.ReactNode[] = []
  let last = 0
  for (const m of text.matchAll(TOKEN)) {
    const at = m.index ?? 0
    if (at > last) parts.push(text.slice(last, at))
    const token = m[0]
    if (token.startsWith("**")) {
      parts.push(
        <strong key={at} className="font-semibold">
          <RichText text={token.slice(2, -2)} onSeek={onSeek} />
        </strong>
      )
    } else if (token.startsWith("`")) {
      parts.push(
        <code
          key={at}
          className="timecode rounded-sm bg-muted px-1 py-0.5 text-[0.85em]"
        >
          {token.slice(1, -1)}
        </code>
      )
    } else if (onSeek) {
      const sec = Number(m[2] ?? 0) * 3600 + Number(m[3]) * 60 + Number(m[4])
      parts.push(
        <button
          key={at}
          type="button"
          onClick={() => onSeek(sec)}
          className="timecode rounded-sm px-0.5 text-[0.85em] underline decoration-marker decoration-2 underline-offset-2 hover:bg-marker/25 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        >
          {token.slice(1, -1)}
        </button>
      )
    } else {
      parts.push(<Fragment key={at}>{token}</Fragment>)
    }
    last = at + token.length
  }
  if (last < text.length) parts.push(text.slice(last))
  return <>{parts}</>
}
