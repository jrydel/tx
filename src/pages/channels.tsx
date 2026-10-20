import { useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  DownloadIcon,
  PlusIcon,
  SearchIcon,
  TvMinimalPlayIcon,
} from "lucide-react"
import { toast } from "sonner"
import { Link } from "wouter"

import { PageHeader } from "@/components/page-header"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
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
  ItemTitle,
} from "@/components/ui/item"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { api, type Channel } from "@/lib/api"
import { ago } from "@/lib/time"

export function ChannelsPage() {
  const qc = useQueryClient()
  const { data: channels, isPending } = useQuery({
    queryKey: ["channels"],
    queryFn: api.channels,
  })
  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: api.settings,
  })
  const [browser, setBrowser] = useState<string | null>(null)
  const [reference, setReference] = useState("")
  const [filter, setFilter] = useState("")

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["channels"] })
    qc.invalidateQueries({ queryKey: ["status"] })
  }
  const importSubs = useMutation({
    mutationFn: (b: string) => api.importSubscriptions(b),
    onSuccess: (n) => {
      toast.success(`Imported ${n} subscriptions`, {
        description: "Switch on Watch for the channels you want analyzed.",
      })
      refresh()
    },
    onError: (e) => toast.error(String(e)),
  })
  const add = useMutation({
    mutationFn: (r: string) => api.addChannel(r),
    onSuccess: (title) => {
      toast.success(`Watching ${title}`)
      setReference("")
      refresh()
    },
    onError: (e) => toast.error(String(e)),
  })
  const watch = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      api.setWatching(id, enabled),
    onMutate: async ({ id, enabled }) => {
      await qc.cancelQueries({ queryKey: ["channels"] })
      qc.setQueryData<Channel[]>(["channels"], (old) =>
        old?.map((c) => (c.id === id ? { ...c, enabled } : c))
      )
    },
    onError: (e) => toast.error(String(e)),
    onSettled: refresh,
  })

  const browserValue = browser ?? settings?.browser ?? "chrome"
  const shown = (channels ?? [])
    .filter((c) => c.title.toLowerCase().includes(filter.trim().toLowerCase()))
    .sort((a, b) => Number(b.enabled) - Number(a.enabled))
  const watchedCount = channels?.filter((c) => c.enabled).length ?? 0

  return (
    <>
      <PageHeader title="Channels" />
      <div className="mx-auto flex w-full max-w-3xl flex-col gap-6 px-4 py-6 md:px-8">
        <div className="grid gap-4 md:grid-cols-2">
          <Card>
            <CardHeader>
              <CardTitle>Import from YouTube</CardTitle>
              <CardDescription>
                Reads your subscriptions from a browser that's signed in to
                YouTube. Nothing leaves this computer except the request to
                YouTube.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="browser">Browser</FieldLabel>
                  <Input
                    id="browser"
                    value={browserValue}
                    onChange={(e) => setBrowser(e.target.value)}
                    spellCheck={false}
                  />
                  <FieldDescription>
                    chrome, firefox, edge, safari or brave. For another profile:{" "}
                    <span className="timecode">chrome:Profile 2</span>
                  </FieldDescription>
                </Field>
              </FieldGroup>
            </CardContent>
            <CardFooter>
              <Button
                disabled={importSubs.isPending}
                onClick={() => importSubs.mutate(browserValue)}
              >
                {importSubs.isPending ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <DownloadIcon data-icon="inline-start" />
                )}
                Import subscriptions
              </Button>
            </CardFooter>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>Add a channel</CardTitle>
              <CardDescription>
                Watch a channel you're not subscribed to. New uploads are
                analyzed from now on.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="reference">Channel</FieldLabel>
                  <Input
                    id="reference"
                    placeholder="@handle or channel URL"
                    value={reference}
                    onChange={(e) => setReference(e.target.value)}
                    onKeyDown={(e) =>
                      e.key === "Enter" &&
                      reference.trim() &&
                      add.mutate(reference)
                    }
                    spellCheck={false}
                  />
                  <FieldDescription>
                    A link to any of the channel's videos works too.
                  </FieldDescription>
                </Field>
              </FieldGroup>
            </CardContent>
            <CardFooter>
              <Button
                variant="outline"
                disabled={!reference.trim() || add.isPending}
                onClick={() => add.mutate(reference)}
              >
                {add.isPending ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <PlusIcon data-icon="inline-start" />
                )}
                Add channel
              </Button>
            </CardFooter>
          </Card>
        </div>

        <Card className="gap-4">
          <CardHeader>
            <CardTitle>Your channels</CardTitle>
            <CardDescription>
              {channels?.length
                ? `Watching ${watchedCount} of ${channels.length}. Uploads from before you switch a channel on are listed under Earlier uploads, not analyzed.`
                : "Import your subscriptions or add a channel to get started."}
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            {isPending &&
              [0, 1, 2, 3].map((i) => (
                <Skeleton key={i} className="h-12 w-full" />
              ))}
            {channels && channels.length > 0 && (
              <>
                <InputGroup>
                  <InputGroupInput
                    placeholder="Filter channels"
                    value={filter}
                    onChange={(e) => setFilter(e.target.value)}
                    aria-label="Filter channels"
                  />
                  <InputGroupAddon>
                    <SearchIcon />
                  </InputGroupAddon>
                </InputGroup>
                <ItemGroup className="rounded-lg border">
                  {shown.map((c) => (
                    <Item
                      key={c.id}
                      size="sm"
                      className="rounded-none border-0 border-b last:border-b-0"
                    >
                      <ItemContent className="min-w-0">
                        <ItemTitle className="w-full">
                          {c.enabled ? (
                            <Link
                              href={`/?channel=${encodeURIComponent(c.id)}`}
                              className="truncate hover:underline"
                            >
                              {c.title}
                            </Link>
                          ) : (
                            <span className="truncate">{c.title}</span>
                          )}
                        </ItemTitle>
                        {c.enabled && (
                          <ItemDescription>
                            {c.analyzed} analyzed · checked {ago(c.checked_at)}
                          </ItemDescription>
                        )}
                      </ItemContent>
                      <ItemActions>
                        <label className="flex items-center gap-2 text-xs text-muted-foreground">
                          {c.enabled ? "Watching" : "Watch"}
                          <Switch
                            checked={c.enabled}
                            onCheckedChange={(enabled) =>
                              watch.mutate({ id: c.id, enabled })
                            }
                            aria-label={`Watch ${c.title}`}
                          />
                        </label>
                      </ItemActions>
                    </Item>
                  ))}
                  {shown.length === 0 && (
                    <p className="p-4 text-sm text-muted-foreground">
                      No channel matches “{filter}”.
                    </p>
                  )}
                </ItemGroup>
              </>
            )}
            {channels && channels.length === 0 && (
              <Empty>
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <TvMinimalPlayIcon />
                  </EmptyMedia>
                  <EmptyTitle>No channels yet</EmptyTitle>
                  <EmptyDescription>
                    Import your YouTube subscriptions or add a channel above.
                  </EmptyDescription>
                </EmptyHeader>
              </Empty>
            )}
          </CardContent>
        </Card>
      </div>
    </>
  )
}
