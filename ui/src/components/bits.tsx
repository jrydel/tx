import { useState, type ComponentProps } from "react"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { openLink, toggleFullscreen, useHost } from "@/host"
import type { Card } from "@/types"

/** A link that opens through the host (a sandboxed frame can't navigate on its own). */
export function ExtLink({ url, className, children, ...props }: { url: string } & ComponentProps<"a">) {
  return (
    <a
      href={url}
      className={cn("cursor-pointer", className)}
      onClick={(e) => {
        e.preventDefault()
        openLink(url)
      }}
      {...props}
    >
      {children}
    </a>
  )
}

/** "Full screen" / "Exit full screen", only where the host offers it. */
export function ExpandButton() {
  const { modes, displayMode } = useHost()
  if (!modes.includes("fullscreen")) return null
  return (
    <Button variant="outline" size="xs" className="rounded-full font-semibold text-muted-foreground" onClick={toggleFullscreen}>
      {displayMode === "fullscreen" ? "Exit full screen" : "Full screen"}
    </Button>
  )
}

export type Kind = "watch" | "skim" | "skip" | "new" | "failed"
export const GROUPS: Record<Kind, string> = { watch: "Watch", skim: "Skim", skip: "Skip", new: "Not scored yet", failed: "Couldn't analyze" }
/** A passed episode (judged from its title and notes) counts as a skip. */
export const kindOf = (v: Card): Kind =>
  v.analyzed ? (v.advice === "watch" || v.advice === "skim" ? v.advice : "skip") : v.state === "passed" ? "skip" : v.state === "failed" ? "failed" : "new"
/** `--k` is the verdict's color; children pick it up with text-(--k) / bg-(--k). */
export const kindColor: Record<Kind, string> = {
  watch: "[--k:var(--watch)]",
  skim: "[--k:var(--skim)]",
  skip: "[--k:var(--skip)]",
  new: "[--k:var(--fresh)]",
  failed: "[--k:var(--destructive)]",
}

/**
 * A video's 16:9 frame. Podcast art is square, so it's contained, not cropped. Without an image (or when it fails) the
 * channel's initial stays. `small` frames are too small for the length badge.
 */
export function Thumb({ v, link, small, className }: { v: Card; link?: boolean; small?: boolean; className?: string }) {
  const [failed, setFailed] = useState(false)
  const inner = (
    <>
      <span aria-hidden className="absolute inset-0 grid place-items-center text-[22px] font-bold text-faint">
        {(v.channel || "?").trim().charAt(0).toUpperCase()}
      </span>
      {v.thumbnail && !failed && (
        <img
          src={v.thumbnail}
          alt=""
          loading="lazy"
          onError={() => setFailed(true)}
          className={cn(
            "absolute inset-0 block size-full",
            v.podcast ? "object-contain" : "object-cover",
            link && "transition-transform duration-250 group-hover/thumb:scale-105"
          )}
        />
      )}
      {v.length && !small && (
        <span className="absolute right-1 bottom-1 rounded-[3px] bg-black/80 px-1 font-mono text-[10.5px] leading-normal font-semibold text-white">
          {v.length}
        </span>
      )}
    </>
  )
  const cls = cn("group/thumb relative block aspect-video overflow-hidden rounded-lg bg-muted", className)
  return link ? (
    <ExtLink url={v.url} className={cls} title={v.podcast ? "Open the episode" : "Play on YouTube"}>
      {inner}
    </ExtLink>
  ) : (
    <span className={cls}>{inner}</span>
  )
}
