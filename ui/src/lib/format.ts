export interface Locale {
  locale?: string
  timeZone?: string
}

function ago(d: Date): string {
  const s = (Date.now() - d.getTime()) / 1000
  const units: [number, string][] = [
    [31536000, "year"],
    [2592000, "month"],
    [604800, "week"],
    [86400, "day"],
    [3600, "hour"],
    [60, "minute"],
  ]
  for (const [n, u] of units) {
    if (s >= n) {
      const k = Math.floor(s / n)
      return `${k} ${u}${k > 1 ? "s" : ""} ago`
    }
  }
  return "just now"
}

/** Release time in the user's locale and time zone: "Oct 15, 2026, 7:35 PM · 6 days ago"; `short` gives "Oct 15". */
export function released(iso: string | number | Date, loc: Locale, short = false): string {
  const d = new Date(iso)
  if (isNaN(d.getTime())) return ""
  const opts: Intl.DateTimeFormatOptions = short
    ? { day: "numeric", month: "short" }
    : { dateStyle: "medium", timeStyle: "short" }
  let exact: string
  try {
    exact = d.toLocaleString(loc.locale, { ...opts, timeZone: loc.timeZone })
  } catch {
    exact = d.toLocaleString(undefined, opts)
  }
  return short ? exact : `${exact} · ${ago(d)}`
}

/** Today as "Fri, 23 Oct". */
export function today(loc: Locale): string {
  const opts: Intl.DateTimeFormatOptions = { weekday: "short", day: "numeric", month: "short" }
  try {
    return new Date().toLocaleDateString(loc.locale, { ...opts, timeZone: loc.timeZone })
  } catch {
    return new Date().toLocaleDateString(undefined, opts)
  }
}

/** Seconds from "mm:ss" or "h:mm:ss". */
export function secs(t?: string | null): number | null {
  return t && /^\d+(:\d+){1,2}$/.test(t) ? t.split(":").reduce((a, p) => a * 60 + Number(p), 0) : null
}

export const plural = (k: number, word: string) => `${k} ${word}${k > 1 ? "s" : ""}`
