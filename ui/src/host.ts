/**
 * MCP Apps host bridge: JSON-RPC over window.parent.postMessage (https://modelcontextprotocol.io, "MCP Apps").
 * Everything the host tells us ends up in one immutable snapshot that React reads with `useHost()`.
 */
import { useSyncExternalStore } from "react"
import type { ToolResult } from "@/types"

export interface HostState {
  /** The host proxies tools/call to tx's MCP server. */
  canCallTools: boolean
  /** The host forwards ui/message into the chat (a bare `message: {}` means it won't). */
  canMessage: boolean
  locale?: string
  timeZone?: string
  displayMode: string
  modes: string[]
  /** The latest tool result's structuredContent, and a counter that bumps on every delivery. */
  output: unknown
  outputSeq: number
}

interface OpenAI {
  toolOutput?: unknown
  theme?: string
  locale?: string
  displayMode?: string
}

let state: HostState = {
  canCallTools: false,
  canMessage: false,
  displayMode: "inline",
  modes: [],
  output: null,
  outputSeq: 0,
}
const listeners = new Set<() => void>()
const set = (patch: Partial<HostState>) => {
  state = { ...state, ...patch }
  listeners.forEach((l) => l())
}

export const useHost = () =>
  useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => state
  )

// ---- JSON-RPC plumbing ----

let nextId = 1
const waiting = new Map<number, { resolve: (v: unknown) => void; reject: (e: unknown) => void }>()

const send = (msg: Record<string, unknown>) => window.parent.postMessage({ jsonrpc: "2.0", ...msg }, "*")

function request<T = unknown>(method: string, params?: unknown): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const id = nextId++
    waiting.set(id, { resolve: resolve as (v: unknown) => void, reject })
    send({ id, method, params })
  })
}

// ---- Actions the view can take through the host ----

export function openLink(url: string) {
  request("ui/open-link", { url }).catch(() => window.open(url, "_blank", "noopener"))
}

export async function toggleFullscreen() {
  try {
    const r = await request<{ mode?: string } | undefined>("ui/request-display-mode", {
      mode: state.displayMode === "fullscreen" ? "inline" : "fullscreen",
    })
    if (r && r.mode) set({ displayMode: r.mode })
  } catch {
    /* the host said no */
  }
}

const textOf = (r: ToolResult) => (r.content ?? []).map((c) => c.text ?? "").join("\n").trim()

/** Call one of tx's tools through the host. Rejects with the tool's own message on `isError`. */
export async function callTool(name: string, args: Record<string, unknown> = {}): Promise<ToolResult> {
  const r = await request<ToolResult>("tools/call", { name, arguments: args })
  if (r?.isError) throw new Error(textOf(r) || `${name} failed`)
  return r ?? {}
}

/** Put a message in the chat as the user. Only when the host forwards them. */
export async function sendMessage(text: string) {
  await request("ui/message", { role: "user", content: { type: "text", text } })
}

export { textOf }

// ---- Host context: theme, sizes, locale, fonts ----

function setTheme(theme: string) {
  document.documentElement.style.colorScheme = theme
  document.documentElement.classList.toggle("dark", theme === "dark")
}

interface HostContext {
  theme?: string
  locale?: string
  timeZone?: string
  displayMode?: string
  availableDisplayModes?: string[]
  containerDimensions?: { height?: number; maxHeight?: number }
  styles?: { variables?: Record<string, string | undefined>; css?: { fonts?: string } }
}

function applyContext(ctx?: HostContext | null) {
  if (!ctx) return
  const root = document.documentElement
  if (ctx.theme) setTheme(ctx.theme)
  const max = ctx.containerDimensions && (ctx.containerDimensions.height || ctx.containerDimensions.maxHeight)
  if (max) root.style.setProperty("--app-h", `${Math.max(360, Math.min(2400, max))}px`)
  const vars = ctx.styles?.variables
  if (vars) for (const [k, v] of Object.entries(vars)) if (v) root.style.setProperty(k, v)
  const fonts = ctx.styles?.css?.fonts
  if (fonts) {
    const s = document.createElement("style")
    s.textContent = fonts
    document.head.append(s)
  }
  const patch: Partial<HostState> = {}
  if (ctx.locale) patch.locale = ctx.locale
  if (ctx.timeZone) patch.timeZone = ctx.timeZone
  if (ctx.displayMode) patch.displayMode = ctx.displayMode
  if (Array.isArray(ctx.availableDisplayModes)) patch.modes = ctx.availableDisplayModes
  set(patch)
}

const deliver = (output: unknown) => set({ output, outputSeq: state.outputSeq + 1 })

function onMessage(e: MessageEvent) {
  const m = e.data
  if (!m || m.jsonrpc !== "2.0") return
  if (m.id != null && !m.method) {
    const w = waiting.get(m.id)
    if (w) {
      waiting.delete(m.id)
      if (m.error) w.reject(m.error)
      else w.resolve(m.result)
    }
    return
  }
  switch (m.method) {
    case "ui/notifications/tool-result":
      deliver(m.params?.structuredContent)
      break
    case "ui/notifications/host-context-changed":
      applyContext(m.params)
      break
    case "ui/resource-teardown":
    case "ping":
      send({ id: m.id, result: {} })
      break
    default:
      if (m.id != null) send({ id: m.id, error: { code: -32601, message: "not supported" } })
  }
}

export function startHost() {
  const dark = window.matchMedia("(prefers-color-scheme: dark)")
  const follow = () => document.documentElement.classList.toggle("dark", dark.matches)
  follow()
  dark.addEventListener("change", () => {
    if (!document.documentElement.style.colorScheme) follow()
  })

  window.addEventListener("message", onMessage)

  // Report the content's height (the body box, not the document's: that is never smaller than the frame, so it could not shrink).
  new ResizeObserver(() => {
    const r = document.documentElement.getBoundingClientRect()
    send({
      method: "ui/notifications/size-changed",
      params: { width: Math.ceil(r.width), height: Math.ceil(document.body.getBoundingClientRect().height) },
    })
  }).observe(document.body)

  request<{ hostCapabilities?: { serverTools?: unknown; message?: { text?: unknown } }; hostContext?: HostContext }>(
    "ui/initialize",
    {
      appInfo: { name: "tx", version: "1" },
      appCapabilities: { availableDisplayModes: ["inline", "fullscreen"] },
      protocolVersion: "2026-01-26",
    }
  ).then(
    (r) => {
      const caps = r?.hostCapabilities ?? {}
      set({ canCallTools: !!caps.serverTools, canMessage: !!caps.message?.text })
      applyContext(r?.hostContext)
      send({ method: "ui/notifications/initialized" })
      // tx is meant to be read full screen (Codex: in its side panel). Hosts that read the tools' preferred display
      // mode open it that way already; for the rest, ask once on load. Exit full screen still works.
      if (state.modes.includes("fullscreen") && state.displayMode !== "fullscreen") void toggleFullscreen()
    },
    () => {}
  )

  // ChatGPT Apps SDK hosts expose the tool result as window.openai.toolOutput.
  const openai = () => (window as unknown as { openai?: OpenAI }).openai
  const fromOpenAI = () => {
    const o = openai()
    if (!o) return
    if (o.theme) setTheme(o.theme)
    if (o.locale || o.displayMode) applyContext({ locale: o.locale, displayMode: o.displayMode })
    if (o.toolOutput && o.toolOutput !== state.output) deliver(o.toolOutput)
  }
  fromOpenAI()
  window.addEventListener("openai:set_globals", fromOpenAI)
}
