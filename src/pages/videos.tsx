import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  ClapperboardIcon,
  SearchIcon,
  TriangleAlertIcon,
  XIcon,
} from "lucide-react"
import { toast } from "sonner"
import { Link, useLocation, useSearch } from "wouter"

import { FilmStrip } from "@/components/film-strip"
import { PageHeader } from "@/components/page-header"
import { ScoreBadge } from "@/components/score-badge"
import { Thumbnail } from "@/components/thumbnail"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group"
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useStatus } from "@/hooks/use-status"
import { parseAnalysis, type Analysis } from "@/lib/analysis"
import { api, type Video, type View } from "@/lib/api"
import { ago, dayLabel, youtubeAt } from "@/lib/time"

const SCORES = [
  { value: "0", label: "Any" },
  { value: "2", label: "2+" },
  { value: "3", label: "3+" },
  { value: "4", label: "4+" },
  { value: "5", label: "5" },
]
const VERDICTS: Record<string, string> = {
  any: "Any advice",
  watch: "Watch fully",
  skim: "Skim a part",
  skip: "Skip it",
}
const SORTS: Record<string, string> = {
  newest: "Newest first",
  score: "Best score first",
}

type Row = { video: Video; analysis: Analysis }

function ChannelLine({ video }: { video: Video }) {
  return (
    <span className="timecode truncate text-[11px] tracking-wide text-muted-foreground uppercase">
      {video.channel} · {ago(video.published)}
    </span>
  )
}

function VideoCard({ row: { video, analysis } }: { row: Row }) {
  const qc = useQueryClient()
  const retry = useMutation({
    mutationFn: () => api.analyze(video.id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["videos"] }),
    onError: (e) => toast.error(String(e)),
  })
  const busy = video.status === "queued" || video.status === "working"
  return (
    <Card className="gap-0 overflow-hidden p-0">
      <div className="grid gap-4 p-4 sm:grid-cols-[13rem_minmax(0,1fr)]">
        <Link
          href={`/v/${video.id}`}
          tabIndex={-1}
          aria-hidden
          className="relative"
        >
          <Thumbnail id={video.id} duration={video.duration} />
          {busy && (
            <span className="absolute inset-0 flex items-center justify-center rounded-md bg-strip/60 text-white">
              <Spinner />
            </span>
          )}
        </Link>
        <div className="flex min-w-0 flex-col gap-1.5">
          <div className="flex items-start gap-3">
            <ChannelLine video={video} />
            <span className="ml-auto shrink-0">
              <ScoreBadge relevance={video.relevance} />
            </span>
          </div>
          <h3 className="slate text-[1.4rem] leading-[1.08]">
            <Link
              href={`/v/${video.id}`}
              className="hover:underline hover:decoration-marker hover:decoration-2 hover:underline-offset-4"
            >
              {video.title}
            </Link>
          </h3>
          {video.verdict && (
            <p className="text-sm text-foreground/90">{video.verdict}</p>
          )}
          {video.status === "done" && analysis.tldr && (
            <p className="line-clamp-2 text-sm text-muted-foreground">
              {analysis.tldr}
            </p>
          )}
          {busy && (
            <p className="text-sm text-muted-foreground">
              {video.status === "working"
                ? "Analyzing now"
                : "Waiting to be analyzed"}
            </p>
          )}
          {video.status === "error" && (
            <Alert variant="destructive" className="mt-1">
              <TriangleAlertIcon />
              <AlertTitle>Analysis failed</AlertTitle>
              <AlertDescription className="line-clamp-2 break-words">
                {video.error}
              </AlertDescription>
              <div className="col-start-2 mt-2">
                <Button
                  size="sm"
                  variant="outline"
                  disabled={retry.isPending}
                  onClick={() => retry.mutate()}
                >
                  Try again
                </Button>
              </div>
            </Alert>
          )}
        </div>
      </div>
      {video.status === "done" && video.duration !== null && (
        <FilmStrip
          className="px-4 pb-4"
          duration={video.duration}
          takeaways={analysis.takeaways}
          watch={analysis.watch}
          onSeek={(sec) => api.openUrl(youtubeAt(video.url, sec))}
        />
      )}
    </Card>
  )
}

function EarlierList({ rows }: { rows: Row[] }) {
  const qc = useQueryClient()
  const analyze = useMutation({
    mutationFn: (id: string) => api.analyze(id),
    onSuccess: () => {
      toast("Queued for analysis")
      qc.invalidateQueries({ queryKey: ["videos"] })
      qc.invalidateQueries({ queryKey: ["status"] })
    },
    onError: (e) => toast.error(String(e)),
  })
  return (
    <ItemGroup className="rounded-xl border bg-card">
      {rows.map(({ video: v }) => (
        <Item
          key={v.id}
          className="rounded-none border-0 border-b last:border-b-0"
        >
          <ItemMedia variant="image" className="h-auto w-28">
            <Thumbnail id={v.id} duration={v.duration} className="w-28" />
          </ItemMedia>
          <ItemContent className="min-w-0">
            <ItemTitle className="line-clamp-2 w-full">{v.title}</ItemTitle>
            <ItemDescription>
              <ChannelLine video={v} />
            </ItemDescription>
          </ItemContent>
          <ItemActions>
            <Button
              size="sm"
              variant="outline"
              disabled={analyze.isPending && analyze.variables === v.id}
              onClick={() => analyze.mutate(v.id)}
            >
              Analyze
            </Button>
          </ItemActions>
        </Item>
      ))}
    </ItemGroup>
  )
}

export function VideosPage() {
  const params = new URLSearchParams(useSearch())
  const [, navigate] = useLocation()
  const { data: status } = useStatus()
  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: api.settings,
  })
  const { data: channels } = useQuery({
    queryKey: ["channels"],
    queryFn: api.channels,
  })

  const view: View = params.get("view") === "earlier" ? "earlier" : "analyzed"
  const q = params.get("q") ?? ""
  const channel = params.get("channel") ?? "all"
  // Default bar: the notification threshold, i.e. "worth your time".
  const min = params.get("min") ?? String(settings?.notify_min ?? 3)
  const verdict = params.get("verdict") ?? "any"
  const sort = params.get("sort") ?? "newest"
  const filtered =
    q !== "" ||
    channel !== "all" ||
    params.has("min") ||
    verdict !== "any" ||
    sort !== "newest"

  const set = (changes: Record<string, string | null>) => {
    const next = new URLSearchParams(params)
    for (const [k, v] of Object.entries(changes)) {
      if (v === null) next.delete(k)
      else next.set(k, v)
    }
    navigate(next.size ? `/?${next}` : "/", { replace: true })
  }

  const { data: videos, isPending } = useQuery({
    queryKey: ["videos", view],
    queryFn: () => api.videos(view),
    refetchInterval: (query) =>
      query.state.data?.some(
        (v) => v.status === "queued" || v.status === "working"
      )
        ? 4_000
        : 30_000,
  })

  const needle = q.trim().toLowerCase()
  const rows: Row[] = (videos ?? [])
    .map((video) => ({ video, analysis: parseAnalysis(video.analysis) }))
    .filter(({ video: v, analysis: a }) => {
      if (channel !== "all" && v.channel_id !== channel) return false
      if (
        needle &&
        ![v.title, v.channel, v.verdict, a.tldr].some((s) =>
          s?.toLowerCase().includes(needle)
        )
      )
        return false
      if (view === "earlier" || v.status !== "done") return true // in progress / failed always show
      if (v.relevance !== null && v.relevance < Number(min)) return false
      return verdict === "any" || a.watch.kind === verdict
    })
    .sort((x, y) =>
      sort === "score"
        ? (y.video.relevance ?? -1) - (x.video.relevance ?? -1)
        : 0
    )

  const groups: [string, Row[]][] = []
  for (const row of rows) {
    const label =
      sort === "score"
        ? `${row.video.relevance ?? "?"}/5`
        : dayLabel(row.video.published)
    const last = groups.at(-1)
    if (last && last[0] === label) last[1].push(row)
    else groups.push([label, [row]])
  }

  const watched = channels?.filter((c) => c.enabled) ?? []
  const channelTitle = channels?.find((c) => c.id === channel)?.title

  return (
    <>
      <PageHeader
        title={channelTitle ?? "Videos"}
        toolbar={
          <div className="flex flex-wrap items-center gap-2">
            <InputGroup className="w-full sm:w-64">
              <InputGroupInput
                placeholder="Search titles and summaries"
                value={q}
                onChange={(e) => set({ q: e.target.value || null })}
                aria-label="Search videos"
              />
              <InputGroupAddon>
                <SearchIcon />
              </InputGroupAddon>
            </InputGroup>
            <Select
              value={channel}
              onValueChange={(v) => set({ channel: v === "all" ? null : v })}
            >
              <SelectTrigger size="sm" className="w-44" aria-label="Channel">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="all">All channels</SelectItem>
                  {watched.map((c) => (
                    <SelectItem key={c.id} value={c.id}>
                      {c.title}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
            {view === "analyzed" && (
              <>
                <ToggleGroup
                  type="single"
                  variant="outline"
                  size="sm"
                  value={min}
                  onValueChange={(v) =>
                    v &&
                    set({
                      min: v === String(settings?.notify_min ?? 3) ? null : v,
                    })
                  }
                  aria-label="Minimum score"
                >
                  {SCORES.map((s) => (
                    <ToggleGroupItem
                      key={s.value}
                      value={s.value}
                      className="timecode px-2.5 text-xs"
                    >
                      {s.label}
                    </ToggleGroupItem>
                  ))}
                </ToggleGroup>
                <Select
                  value={verdict}
                  onValueChange={(v) =>
                    set({ verdict: v === "any" ? null : v })
                  }
                >
                  <SelectTrigger
                    size="sm"
                    className="w-36"
                    aria-label="Watch advice"
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {Object.entries(VERDICTS).map(([value, label]) => (
                        <SelectItem key={value} value={value}>
                          {label}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
                <Select
                  value={sort}
                  onValueChange={(v) =>
                    set({ sort: v === "newest" ? null : v })
                  }
                >
                  <SelectTrigger size="sm" className="w-40" aria-label="Sort">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {Object.entries(SORTS).map(([value, label]) => (
                        <SelectItem key={value} value={value}>
                          {label}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </>
            )}
            {filtered && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  set({
                    q: null,
                    channel: null,
                    min: null,
                    verdict: null,
                    sort: null,
                  })
                }
              >
                <XIcon data-icon="inline-start" />
                Clear filters
              </Button>
            )}
            {videos && (
              <span className="timecode ml-auto text-[11px] text-muted-foreground">
                {rows.length} of {videos.length}
              </span>
            )}
          </div>
        }
      >
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          value={view}
          onValueChange={(v) => v && set({ view: v === "analyzed" ? null : v })}
        >
          <ToggleGroupItem value="analyzed">Analyzed</ToggleGroupItem>
          <ToggleGroupItem value="earlier">Earlier uploads</ToggleGroupItem>
        </ToggleGroup>
      </PageHeader>

      <div className="mx-auto flex w-full max-w-4xl flex-col gap-6 px-4 py-6 md:px-8">
        {status && !status.agent && (
          <Alert variant="destructive">
            <TriangleAlertIcon />
            <AlertTitle>Claude Code isn't installed</AlertTitle>
            <AlertDescription>
              tx uses Claude Code to analyze videos. Install it from
              claude.com/claude-code, sign in once in a terminal, then click
              Check now.
            </AlertDescription>
          </Alert>
        )}
        {status && status.watched > 0 && !status.profile_set && (
          <Alert>
            <AlertTitle>Describe what you want to learn</AlertTitle>
            <AlertDescription>
              <span>
                Scores are generic until you do.{" "}
                <Link href="/settings" className="underline underline-offset-2">
                  Open Settings
                </Link>
              </span>
            </AlertDescription>
          </Alert>
        )}

        {isPending &&
          [0, 1, 2].map((i) => (
            <Skeleton key={i} className="h-44 w-full rounded-xl" />
          ))}

        {videos && rows.length === 0 && (
          <Empty className="border">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <ClapperboardIcon />
              </EmptyMedia>
              <EmptyTitle>
                {status?.watched === 0
                  ? "Pick channels to watch"
                  : videos.length > 0
                    ? "No videos match these filters"
                    : view === "earlier"
                      ? "No earlier uploads"
                      : "Nothing analyzed yet"}
              </EmptyTitle>
              <EmptyDescription>
                {status?.watched === 0
                  ? "tx analyzes new uploads from the channels you watch and tells you which ones deserve your time."
                  : videos.length > 0
                    ? `The score filter starts at your notification threshold (${settings?.notify_min ?? 3}+). Lower it to see more.`
                    : view === "earlier"
                      ? "Uploads published before you started watching a channel show up here."
                      : "New uploads from watched channels appear here once they're analyzed. Older ones are under Earlier uploads."}
              </EmptyDescription>
            </EmptyHeader>
            <EmptyContent>
              {status?.watched === 0 ? (
                <Button asChild>
                  <Link href="/channels">Choose channels</Link>
                </Button>
              ) : videos.length > 0 ? (
                <Button
                  variant="outline"
                  onClick={() =>
                    set({ q: null, channel: null, min: "0", verdict: null })
                  }
                >
                  Show every analyzed video
                </Button>
              ) : view !== "earlier" ? (
                <Button
                  variant="outline"
                  onClick={() => set({ view: "earlier" })}
                >
                  Show earlier uploads
                </Button>
              ) : null}
            </EmptyContent>
          </Empty>
        )}

        {view === "earlier" && rows.length > 0 && <EarlierList rows={rows} />}

        {view === "analyzed" &&
          groups.map(([label, group]) => (
            <section
              key={label}
              className="flex flex-col gap-3"
              aria-label={label}
            >
              <h2 className="timecode text-[11px] tracking-[0.14em] text-muted-foreground uppercase">
                {label}
              </h2>
              {group.map((row) => (
                <VideoCard key={row.video.id} row={row} />
              ))}
            </section>
          ))}
      </div>
    </>
  )
}
