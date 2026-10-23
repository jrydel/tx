import { useEffect, useRef, useState, type ReactNode } from "react"
import { Command as CommandPrimitive } from "cmdk"
import { LinkIcon, SearchIcon } from "lucide-react"
import { buttonVariants } from "@/components/ui/button"
import { Command, CommandGroup, CommandItem, CommandList } from "@/components/ui/command"
import { InputGroup, InputGroupAddon } from "@/components/ui/input-group"
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { cn } from "@/lib/utils"
import { callTool } from "@/host"
import type { Channel, SourceResult } from "@/types"
import { FollowingMark, SourceLine } from "./source-row"

/** A link or an @handle: pass it to tx as typed rather than searching for it. */
const looksLikeRef = (q: string) => /^(https?:\/\/|www\.|@)/i.test(q) || /^[\w.-]+\.\w{2,}\//.test(q)

interface Row {
  key: string
  ref: string
  title: string
  subtitle: string
  kind: "youtube" | "podcast"
  image: string | null
  following: boolean
}

/**
 * Search box with autocomplete: what's already in the user's list matches instantly; after a pause, tx searches
 * YouTube and Apple Podcasts. Enter or a click follows. A pasted link or @handle can be followed as typed.
 */
export function SourceSearch({
  channels,
  canSearch,
  onFollow,
}: {
  channels: Channel[]
  canSearch: boolean
  /** Follow `ref`; resolves true when it was followed. */
  onFollow: (ref: string, name: string) => Promise<boolean>
}) {
  const [query, setQuery] = useState("")
  const [open, setOpen] = useState(false)
  const [results, setResults] = useState<SourceResult[]>([])
  const [searching, setSearching] = useState(false)
  const [searchError, setSearchError] = useState<string | null>(null)
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set())
  const [followed, setFollowed] = useState<ReadonlySet<string>>(new Set())
  const anchor = useRef<HTMLDivElement>(null)
  const q = query.trim()

  // Debounced remote search. The previous results stay up while the next ones load.
  useEffect(() => {
    setSearchError(null)
    if (q.length < 2 || !canSearch || looksLikeRef(q)) {
      setSearching(false)
      if (q.length < 2) setResults([])
      return
    }
    setSearching(true)
    let live = true
    const t = setTimeout(async () => {
      try {
        const r = await callTool("search_sources", { query: q })
        if (live) setResults((r.structuredContent as { results?: SourceResult[] } | undefined)?.results ?? [])
      } catch (e) {
        if (live) setSearchError(e instanceof Error ? e.message : "Search failed.")
      } finally {
        if (live) setSearching(false)
      }
    }, 300)
    return () => {
      live = false
      clearTimeout(t)
    }
  }, [q, canSearch])

  const lower = q.toLowerCase()
  const local: Row[] = q
    ? channels
        .filter((c) => c.title.toLowerCase().includes(lower))
        .slice(0, 4)
        .map((c) => ({
          key: `local:${c.id}`,
          ref: c.id,
          title: c.title,
          subtitle: c.subtitle ?? (c.watching ? "Following" : "In your list"),
          kind: c.podcast ? "podcast" : "youtube",
          image: c.thumbnail ?? null,
          following: c.watching,
        }))
    : []
  const seen = new Set(local.map((r) => r.title.toLowerCase()))
  const remote: Row[] = results
    .filter((r) => !seen.has(r.title.toLowerCase()))
    .map((r) => ({
      key: `remote:${r.kind}:${r.ref}`,
      ref: r.ref,
      title: r.title,
      subtitle: r.subtitle,
      kind: r.kind,
      image: r.thumbnail,
      following: r.watching,
    }))
  const direct = q && (looksLikeRef(q) || !canSearch || (!searching && !searchError && q.length >= 2 && !local.length && !remote.length))

  const follow = async (ref: string, name: string, key: string) => {
    if (pending.has(key)) return
    setPending((p) => new Set(p).add(key))
    const ok = await onFollow(ref, name)
    setPending((p) => {
      const n = new Set(p)
      n.delete(key)
      return n
    })
    if (ok) setFollowed((f) => new Set(f).add(key))
  }

  const row = (r: Row) => {
    const isFollowing = r.following || followed.has(r.key)
    const busy = pending.has(r.key)
    return (
      <CommandItem key={r.key} value={r.key} onSelect={() => !isFollowing && follow(r.ref, r.title, r.key)} className="gap-3 py-2">
        <SourceLine s={{ title: r.title, kind: r.kind, image: r.image, subtitle: r.subtitle }} />
        {isFollowing ? (
          <FollowingMark />
        ) : (
          <span className={cn(buttonVariants({ variant: "secondary", size: "xs" }), "w-[84px]")}>{busy ? <Spinner className="size-3" /> : "Follow"}</span>
        )}
      </CommandItem>
    )
  }

  const note = (children: ReactNode) => <div className="px-2.5 py-2 text-[13px] text-muted-foreground">{children}</div>

  return (
    <Popover open={open && q.length > 0} onOpenChange={setOpen}>
      <Command shouldFilter={false} loop label="Search YouTube channels and podcasts" className="overflow-visible bg-transparent p-0">
        <PopoverAnchor asChild>
          <div ref={anchor}>
            <InputGroup className="h-9">
              <CommandPrimitive.Input
                data-slot="input-group-control"
                value={query}
                onValueChange={(v) => {
                  setQuery(v)
                  setOpen(true)
                }}
                onFocus={() => setOpen(true)}
                placeholder="Search YouTube channels and podcasts"
                aria-label="Search YouTube channels and podcasts"
                className="h-full flex-1 bg-transparent px-2.5 text-sm outline-hidden placeholder:text-muted-foreground"
              />
              <InputGroupAddon>
                <SearchIcon />
              </InputGroupAddon>
              {searching && (
                <InputGroupAddon align="inline-end">
                  <Spinner />
                </InputGroupAddon>
              )}
            </InputGroup>
          </div>
        </PopoverAnchor>
        <PopoverContent
          align="start"
          className="w-(--radix-popover-trigger-width) gap-0 p-0"
          onOpenAutoFocus={(e) => e.preventDefault()}
          onInteractOutside={(e) => {
            if (anchor.current?.contains(e.target as Node)) e.preventDefault()
          }}
        >
          <CommandList className="max-h-[min(22rem,var(--radix-popover-content-available-height))] p-1">
            {local.length > 0 && <CommandGroup heading="In your list">{local.map(row)}</CommandGroup>}
            {remote.length > 0 && <CommandGroup heading="YouTube and podcasts">{remote.map(row)}</CommandGroup>}
            {direct && (
              <CommandGroup>
                <CommandItem value="direct" onSelect={() => follow(q, q, "direct")} className="gap-2.5 py-1.5">
                  <LinkIcon className="ml-2.5 size-4 text-muted-foreground" />
                  <span className="min-w-0 flex-1 truncate text-[13.5px]">
                    Follow <b className="font-semibold">{q}</b>
                  </span>
                  <span className={cn(buttonVariants({ variant: "secondary", size: "xs" }), "w-[72px]")}>
                    {pending.has("direct") ? <Spinner className="size-3" /> : followed.has("direct") ? "Following" : "Follow"}
                  </span>
                </CommandItem>
              </CommandGroup>
            )}
            {searching && !remote.length && note("Searching YouTube and Apple Podcasts…")}
            {searchError && <div role="alert" className="px-2.5 py-2 text-[13px] text-destructive">{searchError}</div>}
            {!searching && !searchError && !local.length && !remote.length && !direct &&
              note("No matches. Try a link or an @handle.")}
          </CommandList>
        </PopoverContent>
      </Command>
    </Popover>
  )
}
