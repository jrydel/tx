import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { ClapperboardIcon, TriangleAlertIcon, XIcon } from "lucide-react"
import { toast } from "sonner"
import { Link, useLocation, useSearch } from "wouter"

import { FilmStrip } from "@/components/film-strip"
import { PageHeader } from "@/components/page-header"
import { ScoreBadge } from "@/components/score-badge"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemTitle,
} from "@/components/ui/item"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useStatus } from "@/hooks/use-status"
import { parseAnalysis } from "@/lib/analysis"
import { api, type Video, type View } from "@/lib/api"
import { ago, dayLabel, timecode, youtubeAt } from "@/lib/time"

const VIEWS: { value: View; label: string }[] = [
  { value: "worth", label: "Worth your time" },
  { value: "all", label: "All analyzed" },
  { value: "earlier", label: "Earlier uploads" },
]

function ChannelLine({ video }: { video: Video }) {
  const parts = [
    video.channel,
    video.duration !== null ? timecode(video.duration) : null,
    ago(video.published),
  ].filter(Boolean)
  return (
    <span className="timecode truncate text-[11px] tracking-wide text-muted-foreground uppercase">
      {parts.join(" · ")}
    </span>
  )
}

function VideoCard({ video }: { video: Video }) {
  const qc = useQueryClient()
  const retry = useMutation({
    mutationFn: () => api.analyze(video.id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["videos"] }),
    onError: (e) => toast.error(String(e)),
  })
  const analysis = parseAnalysis(video.analysis)

  return (
    <Card className="gap-3 py-4">
      <CardHeader className="gap-1.5 px-4 md:px-5">
        <ChannelLine video={video} />
        <CardTitle className="slate text-[1.35rem] leading-[1.1]">
          <Link
            href={`/v/${video.id}`}
            className="hover:underline hover:decoration-marker hover:decoration-2 hover:underline-offset-4"
          >
            {video.title}
          </Link>
        </CardTitle>
        {video.verdict && (
          <CardDescription className="text-foreground/85">
            {video.verdict}
          </CardDescription>
        )}
        <CardAction>
          <ScoreBadge relevance={video.relevance} />
        </CardAction>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 px-4 md:px-5">
        {video.status === "done" && video.duration !== null && (
          <FilmStrip
            duration={video.duration}
            takeaways={analysis.takeaways}
            watch={analysis.watch}
            onSeek={(sec) => api.openUrl(youtubeAt(video.url, sec))}
          />
        )}
        {video.status === "done" && analysis.tldr && (
          <p className="line-clamp-2 text-sm text-muted-foreground">
            {analysis.tldr}
          </p>
        )}
        {(video.status === "queued" || video.status === "working") && (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Spinner />
            {video.status === "working"
              ? "Analyzing now"
              : "Waiting to be analyzed"}
          </div>
        )}
        {video.status === "error" && (
          <Alert variant="destructive">
            <TriangleAlertIcon />
            <AlertTitle>Analysis failed</AlertTitle>
            <AlertDescription className="line-clamp-3 break-words">
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
      </CardContent>
    </Card>
  )
}

function EarlierList({ videos }: { videos: Video[] }) {
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
    <ItemGroup className="rounded-lg border bg-card">
      {videos.map((v) => (
        <Item
          key={v.id}
          size="sm"
          className="rounded-none border-0 border-b last:border-b-0"
        >
          <ItemContent className="min-w-0">
            <ItemTitle className="w-full">
              <span className="truncate">{v.title}</span>
            </ItemTitle>
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

function byDay(videos: Video[]): [string, Video[]][] {
  const groups = new Map<string, Video[]>()
  for (const v of videos) {
    const label = dayLabel(v.published)
    groups.set(label, [...(groups.get(label) ?? []), v])
  }
  return [...groups]
}

export function VideosPage() {
  const search = new URLSearchParams(useSearch())
  const [, navigate] = useLocation()
  const view = (search.get("view") as View | null) ?? "worth"
  const channel = search.get("channel") ?? undefined
  const { data: status } = useStatus()
  const { data: channels } = useQuery({
    queryKey: ["channels"],
    queryFn: api.channels,
  })
  const { data: videos, isPending } = useQuery({
    queryKey: ["videos", view, channel],
    queryFn: () => api.videos(view, channel),
    refetchInterval: (q) =>
      q.state.data?.some((v) => v.status === "queued" || v.status === "working")
        ? 4_000
        : 30_000,
  })
  const channelTitle = channel
    ? channels?.find((c) => c.id === channel)?.title
    : undefined
  const go = (next: { view?: View; channel?: string | null }) => {
    const p = new URLSearchParams()
    const v = next.view ?? view
    const c = next.channel === undefined ? channel : next.channel
    if (v !== "worth") p.set("view", v)
    if (c) p.set("channel", c)
    navigate(p.size ? `/?${p}` : "/")
  }

  return (
    <>
      <PageHeader
        title={
          channelTitle ? (
            <span className="flex items-center gap-2">
              <span className="truncate">{channelTitle}</span>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Show all channels"
                onClick={() => go({ channel: null })}
              >
                <XIcon />
              </Button>
            </span>
          ) : (
            "Videos"
          )
        }
      >
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          value={view}
          onValueChange={(v) => v && go({ view: v as View })}
        >
          {VIEWS.map((v) => (
            <ToggleGroupItem key={v.value} value={v.value}>
              {v.label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </PageHeader>

      <div className="mx-auto flex w-full max-w-3xl flex-col gap-6 px-4 py-6 md:px-8">
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

        {isPending && (
          <div className="flex flex-col gap-4">
            {[0, 1, 2].map((i) => (
              <Skeleton key={i} className="h-36 w-full rounded-xl" />
            ))}
          </div>
        )}

        {videos && videos.length === 0 && (
          <Empty className="border">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <ClapperboardIcon />
              </EmptyMedia>
              <EmptyTitle>
                {status?.watched === 0
                  ? "Pick channels to watch"
                  : view === "earlier"
                    ? "No earlier uploads"
                    : view === "worth"
                      ? "Nothing worth your time yet"
                      : "Nothing analyzed yet"}
              </EmptyTitle>
              <EmptyDescription>
                {status?.watched === 0
                  ? "tx analyzes new uploads from the channels you watch and tells you which ones deserve your time."
                  : view === "earlier"
                    ? "Uploads published before you started watching a channel show up here."
                    : "New uploads from watched channels appear here once they're analyzed. Older ones are under Earlier uploads."}
              </EmptyDescription>
            </EmptyHeader>
            {status?.watched === 0 ? (
              <EmptyContent>
                <Button asChild>
                  <Link href="/channels">Choose channels</Link>
                </Button>
              </EmptyContent>
            ) : view !== "earlier" ? (
              <EmptyContent>
                <Button
                  variant="outline"
                  onClick={() => go({ view: "earlier" })}
                >
                  Show earlier uploads
                </Button>
              </EmptyContent>
            ) : null}
          </Empty>
        )}

        {videos && videos.length > 0 && view === "earlier" && (
          <EarlierList videos={videos} />
        )}

        {videos &&
          videos.length > 0 &&
          view !== "earlier" &&
          byDay(videos).map(([day, group]) => (
            <section key={day} className="flex flex-col gap-3" aria-label={day}>
              <h2 className="timecode text-[11px] tracking-[0.14em] text-muted-foreground uppercase">
                {day}
              </h2>
              {group.map((v) => (
                <VideoCard key={v.id} video={v} />
              ))}
            </section>
          ))}
      </div>
    </>
  )
}
