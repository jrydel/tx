import { RichText } from "@/components/rich-text"

type Block =
  | { kind: "p"; text: string }
  | { kind: "h"; text: string }
  | { kind: "ul"; items: string[] }
  | { kind: "ol"; items: string[] }

function blocks(md: string): Block[] {
  const out: Block[] = []
  let para: string[] = []
  const flush = () => {
    if (para.length) out.push({ kind: "p", text: para.join(" ") })
    para = []
  }
  for (const raw of md.split("\n")) {
    const line = raw.trim()
    const heading = line.match(/^#{1,6}\s+(.*)$/)
    const bullet = line.match(/^[-*•]\s+(.*)$/)
    const numbered = line.match(/^\d+[.)]\s+(.*)$/)
    const last = out.at(-1)
    if (!line) flush()
    else if (heading) {
      flush()
      out.push({ kind: "h", text: heading[1] })
    } else if (bullet || numbered) {
      flush()
      const kind = bullet ? "ul" : "ol"
      const text = (bullet ?? numbered)![1]
      if (last && last.kind === kind) last.items.push(text)
      else out.push({ kind, items: [text] })
    } else para.push(line)
  }
  flush()
  return out
}

/**
 * Markdown as agents write it: paragraphs, headings, lists, **bold**, `code`, [mm:ss] links.
 * Rendered as React nodes (never HTML) because answers quote untrusted transcripts.
 */
export function Markdown({
  text,
  onSeek,
}: {
  text: string
  onSeek?: (seconds: number) => void
}) {
  return (
    <div className="flex flex-col gap-2.5 leading-relaxed">
      {blocks(text).map((b, i) => {
        if (b.kind === "h" || b.kind === "p") {
          return (
            <p key={i} className={b.kind === "h" ? "font-semibold" : undefined}>
              <RichText text={b.text} onSeek={onSeek} />
            </p>
          )
        }
        const List = b.kind === "ul" ? "ul" : "ol"
        return (
          <List
            key={i}
            className={
              b.kind === "ul"
                ? "flex list-disc flex-col gap-1 pl-5"
                : "flex list-decimal flex-col gap-1 pl-5"
            }
          >
            {b.items.map((item, j) => (
              <li key={j}>
                <RichText text={item} onSeek={onSeek} />
              </li>
            ))}
          </List>
        )
      })}
    </div>
  )
}
