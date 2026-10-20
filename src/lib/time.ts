export function timecode(sec: number): string {
  const s = Math.max(0, Math.floor(sec))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const ss = String(s % 60).padStart(2, "0")
  return h
    ? `${h}:${String(m).padStart(2, "0")}:${ss}`
    : `${String(m).padStart(2, "0")}:${ss}`
}

const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" })
const startOfDay = (d: Date) =>
  new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()

export function ago(iso: string | null): string {
  if (!iso) return "never"
  const then = new Date(iso)
  const diff = (then.getTime() - Date.now()) / 1000
  const abs = Math.abs(diff)
  if (abs < 60) return "just now"
  if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute")
  // Past midnight, count calendar days so this agrees with the day headers ("Yesterday").
  const days = Math.round(
    (startOfDay(then) - startOfDay(new Date())) / 86400000
  )
  if (days === 0) return rtf.format(Math.round(diff / 3600), "hour")
  return rtf.format(days, "day")
}

export function dayLabel(iso: string): string {
  const d = new Date(iso)
  const days = Math.round((startOfDay(new Date()) - startOfDay(d)) / 86400000)
  if (days === 0) return "Today"
  if (days === 1) return "Yesterday"
  return d.toLocaleDateString(undefined, {
    weekday: "short",
    day: "numeric",
    month: "short",
  })
}

export function youtubeAt(url: string, sec: number): string {
  return `${url}${url.includes("?") ? "&" : "?"}t=${Math.floor(sec)}s`
}
