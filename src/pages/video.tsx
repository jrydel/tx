import { useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  ArrowLeftIcon,
  ExternalLinkIcon,
  PlayIcon,
  RotateCcwIcon,
  ScrollTextIcon,
  TriangleAlertIcon,
} from "lucide-react"
import { toast } from "sonner"
import { Link, useParams } from "wouter"

import { FilmStrip } from "@/components/film-strip"
import { PageHeader } from "@/components/page-header"
import { RichText } from "@/components/rich-text"
import { ScoreBadge } from "@/components/score-badge"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { parseAnalysis } from "@/lib/analysis"
import { api } from "@/lib/api"
import { timecode, youtubeAt } from "@/lib/time"

function TranscriptSheet({
  id,
  onSeek,
}: {
  id: string
  onSeek: (sec: number) => void
}) {
  const [open, setOpen] = useState(false)
  const { data, isPending, error } = useQuery({
    queryKey: ["transcript", id],
    queryFn: () => api.transcript(id),
    enabled: open,
  })
  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger asChild>
        <Button variant="outline">
          <ScrollTextIcon data-icon="inline-start" />
          Transcript
        </Button>
      </SheetTrigger>
      <SheetContent side="right" className="w-full gap-0 sm:max-w-xl">
        <SheetHeader className="border-b">
          <SheetTitle className="slate text-xl">Transcript</SheetTitle>
          <SheetDescription>{data?.source ?? "Loading…"}</SheetDescription>
        </SheetHeader>
        <ScrollArea className="min-h-0 flex-1">
          <div className="flex flex-col gap-4 p-4">
            {isPending &&
              [0, 1, 2, 3].map((i) => (
                <Skeleton key={i} className="h-20 w-full" />
              ))}
            {error && (
              <p className="text-sm text-destructive">{String(error)}</p>
            )}
            {data?.paragraphs.map((p) => (
              <p
                key={p.start}
                className="grid grid-cols-[4.5rem_1fr] gap-3 text-sm leading-relaxed"
              >
                <button
                  type="button"
                  onClick={() => {
                    onSeek(p.start)
                    setOpen(false)
                  }}
                  className="timecode h-fit rounded-sm text-left text-xs text-muted-foreground hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
                >
                  {timecode(p.start)}
                </button>
                <span>{p.text}</span>
              </p>
            ))}
          </div>
        </ScrollArea>
      </SheetContent>
    </Sheet>
  )
}

export function VideoPage() {
  const { id } = useParams<{ id: string }>()
  const qc = useQueryClient()
  const { data: video, error } = useQuery({
    queryKey: ["video", id],
    queryFn: () => api.video(id),
    refetchInterval: (q) =>
      q.state.data && ["queued", "working"].includes(q.state.data.status)
        ? 3_000
        : false,
  })
  const reanalyze = useMutation({
    mutationFn: () => api.analyze(id),
    onSuccess: () => {
      toast("Queued for analysis")
      qc.invalidateQueries({ queryKey: ["video", id] })
      qc.invalidateQueries({ queryKey: ["status"] })
    },
    onError: (e) => toast.error(String(e)),
  })

  if (error) {
    return (
      <>
        <PageHeader title="Video" />
        <p className="px-8 py-6 text-sm text-destructive">{String(error)}</p>
      </>
    )
  }
  if (!video) {
    return (
      <>
        <PageHeader title={<Skeleton className="h-7 w-64" />} />
        <div className="mx-auto w-full max-w-6xl px-4 py-6 md:px-8">
          <Skeleton className="h-64 w-full" />
        </div>
      </>
    )
  }

  const a = parseAnalysis(video.analysis)
  const busy = video.status === "queued" || video.status === "working"
  // YouTube refuses embeds from the app's tauri:// origin (error 153), so timestamps open the browser.
  const seek = (sec: number) => api.openUrl(youtubeAt(video.url, sec))

  return (
    <>
      <PageHeader title={video.channel}>
        <Button variant="ghost" size="sm" asChild>
          <Link href="/">
            <ArrowLeftIcon data-icon="inline-start" />
            Videos
          </Link>
        </Button>
      </PageHeader>
      <article className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-4 py-6 md:px-8">
        <header className="flex flex-col gap-3">
          <span className="timecode text-[11px] tracking-wide text-muted-foreground uppercase">
            {video.channel}
            {video.duration !== null && ` · ${timecode(video.duration)}`} ·{" "}
            {new Date(video.published).toLocaleDateString()}
          </span>
          <h2 className="slate text-4xl text-balance md:text-5xl">
            {video.title}
          </h2>
          <div className="flex flex-wrap items-center gap-3">
            <ScoreBadge relevance={video.relevance} />
            {video.verdict && (
              <p className="text-lg leading-snug text-pretty">
                {video.verdict}
              </p>
            )}
          </div>
          <div className="flex flex-wrap gap-2">
            <Button onClick={() => api.openUrl(video.url)}>
              <ExternalLinkIcon data-icon="inline-start" />
              Open on YouTube
            </Button>
            <TranscriptSheet id={video.id} onSeek={seek} />
            <Button
              variant="outline"
              disabled={busy || reanalyze.isPending}
              onClick={() => reanalyze.mutate()}
            >
              {busy ? (
                <Spinner data-icon="inline-start" />
              ) : (
                <RotateCcwIcon data-icon="inline-start" />
              )}
              {busy
                ? "Analyzing"
                : video.status === "done"
                  ? "Re-analyze"
                  : "Analyze"}
            </Button>
          </div>
        </header>

        {video.status === "done" && video.duration !== null && (
          <FilmStrip
            size="lg"
            duration={video.duration}
            takeaways={a.takeaways}
            watch={a.watch}
            onSeek={seek}
          />
        )}

        {video.status === "error" && (
          <Alert variant="destructive">
            <TriangleAlertIcon />
            <AlertTitle>Analysis failed</AlertTitle>
            <AlertDescription className="break-words">
              {video.error}
            </AlertDescription>
          </Alert>
        )}

        <div className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_minmax(0,26rem)]">
          <div className="flex min-w-0 flex-col gap-8">
            {a.tldr && (
              <section className="flex flex-col gap-2">
                <h3 className="timecode text-[11px] tracking-[0.14em] text-muted-foreground uppercase">
                  TL;DR
                </h3>
                <p className="text-base leading-relaxed">
                  <RichText text={a.tldr} onSeek={seek} />
                </p>
              </section>
            )}
            {a.takeaways.length > 0 && (
              <section className="flex flex-col gap-2">
                <h3 className="timecode text-[11px] tracking-[0.14em] text-muted-foreground uppercase">
                  Takeaways for you
                </h3>
                <ol className="flex flex-col">
                  {/* In timeline order, so the list reads alongside the film strip and the player. */}
                  {[...a.takeaways]
                    .sort((x, y) => (x.at ?? Infinity) - (y.at ?? Infinity))
                    .map((t, i) => (
                      <li
                        key={i}
                        className="grid grid-cols-[4.5rem_1fr] gap-3 border-b py-3 text-sm leading-relaxed last:border-b-0"
                      >
                        {t.at !== null ? (
                          <button
                            type="button"
                            onClick={() => seek(t.at!)}
                            className="timecode h-fit w-fit rounded-sm border-l-[3px] border-marker pl-2 text-xs hover:bg-marker/20 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
                            aria-label={`Play from ${timecode(t.at)}`}
                          >
                            {timecode(t.at)}
                          </button>
                        ) : (
                          <span />
                        )}
                        <span>
                          <RichText text={t.text} onSeek={seek} />
                        </span>
                      </li>
                    ))}
                </ol>
              </section>
            )}
            {a.watch.text && (
              <section className="flex flex-col gap-2">
                <h3 className="timecode text-[11px] tracking-[0.14em] text-muted-foreground uppercase">
                  Worth watching?
                </h3>
                <p className="text-base leading-relaxed">
                  <RichText text={a.watch.text} onSeek={seek} />
                </p>
              </section>
            )}
            {busy && (
              <p className="flex items-center gap-2 text-sm text-muted-foreground">
                <Spinner />{" "}
                {video.status === "working"
                  ? "Analyzing now"
                  : "Waiting to be analyzed"}
              </p>
            )}
          </div>
          <aside className="flex flex-col gap-3 lg:sticky lg:top-20 lg:self-start">
            <button
              type="button"
              onClick={() => api.openUrl(video.url)}
              className="group/thumb relative aspect-video w-full overflow-hidden rounded-lg bg-strip focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
              aria-label={`Watch ${video.title} on YouTube`}
            >
              <img
                src={`https://i.ytimg.com/vi/${video.id}/hqdefault.jpg`}
                alt=""
                className="size-full object-cover opacity-90 transition-opacity group-hover/thumb:opacity-100"
              />
              <span className="absolute inset-x-0 bottom-0 flex items-center gap-2 bg-gradient-to-t from-black/80 to-transparent p-3 text-sm text-white">
                <PlayIcon className="size-4 fill-current" aria-hidden />
                Watch on YouTube
              </span>
            </button>
            <p className="text-xs text-muted-foreground">
              Every timestamp on this page opens the video at that moment.
            </p>
            <p className="text-xs text-muted-foreground">
              Ask follow-up questions in a terminal:{" "}
              <code className="timecode break-all">tx "{video.url}" -c</code>
            </p>
          </aside>
        </div>
      </article>
    </>
  )
}
