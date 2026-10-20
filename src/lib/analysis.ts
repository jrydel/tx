// The analysis is markdown with a fixed shape (see the analysis prompt in the engine):
// ## TL;DR / ## Takeaways for you / ## Worth watching?

export const TIMESTAMP = /\[(?:(\d+):)?(\d{1,2}):(\d{2})\]/g

export type Takeaway = { text: string; at: number | null }
export type WatchAdvice = {
  kind: "skip" | "skim" | "watch" | null
  from: number | null
  to: number | null
  text: string
}
export type Analysis = {
  tldr: string
  takeaways: Takeaway[]
  watch: WatchAdvice
}

export function toSeconds(m: RegExpMatchArray): number {
  return Number(m[1] ?? 0) * 3600 + Number(m[2]) * 60 + Number(m[3])
}

function sections(md: string): Map<string, string> {
  const out = new Map<string, string>()
  let key = ""
  for (const line of md.split("\n")) {
    const h = line.match(/^#{1,6}\s+(.+?)\s*$/)
    if (h) {
      key = h[1].toLowerCase()
      out.set(key, "")
    } else if (key) {
      out.set(key, `${out.get(key)}${line}\n`)
    }
  }
  return out
}

function find(s: Map<string, string>, prefix: string): string {
  for (const [k, v] of s) if (k.startsWith(prefix)) return v.trim()
  return ""
}

export function parseAnalysis(md: string | null): Analysis {
  const s = sections(md ?? "")
  const takeaways = find(s, "takeaways")
    .split("\n")
    .map((l) => l.match(/^\s*[-*•]\s+(.*)$/)?.[1])
    .filter((l): l is string => Boolean(l))
    .map((l) => {
      const first = l.match(new RegExp(TIMESTAMP.source))
      // A trailing timestamp is shown as the item's timecode; drop it from the prose.
      const text = l
        .replace(/(\s*\[(?:\d+:)?\d{1,2}:\d{2}\][,\s]*)+[.]?\s*$/, "")
        .trim()
      return { text, at: first ? toSeconds(first) : null }
    })
  const watchText = find(s, "worth watching")
  const lower = watchText.toLowerCase()
  const kind = lower.startsWith("skip")
    ? "skip"
    : lower.includes("skim")
      ? "skim"
      : lower.includes("watch")
        ? "watch"
        : null
  const [from = null, to = null] =
    kind === "skim" ? [...watchText.matchAll(TIMESTAMP)].map(toSeconds) : []
  return {
    tldr: find(s, "tl;dr"),
    takeaways,
    watch: { kind, from, to, text: watchText },
  }
}
