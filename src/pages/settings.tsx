import { useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { toast } from "sonner"

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
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldSeparator,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
} from "@/components/ui/input-group"
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
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useAppUpdate, useAppVersion } from "@/hooks/use-app-update"
import { api, type Settings } from "@/lib/api"

const PROFILE_EXAMPLE =
  "Example: I'm a backend engineer building LLM features. I want concrete techniques for agents, RAG and evals, and notable model or tooling launches. Skip hype, drama, and beginner tutorials."

const MODEL_NOTES: Record<string, string> = {
  tiny: "fastest, roughest",
  base: "fast",
  small: "balanced",
  medium: "accurate, slow without a GPU",
  "large-v3-turbo": "most accurate; best with a GPU",
}

export function SettingsPage() {
  const qc = useQueryClient()
  const { data: saved } = useQuery({
    queryKey: ["settings"],
    queryFn: api.settings,
  })
  const { data: models } = useQuery({
    queryKey: ["models"],
    queryFn: api.models,
  })
  const { data: autostart } = useQuery({
    queryKey: ["autostart"],
    queryFn: api.autostart,
  })
  // Only unsaved edits live in state; the form shows them over the saved settings.
  const [edits, setEdits] = useState<Settings | null>(null)
  const draft = edits ?? saved ?? null

  const save = useMutation({
    mutationFn: (s: Settings) => api.saveSettings(s),
    onSuccess: (s) => {
      qc.setQueryData(["settings"], s)
      setEdits(null)
      qc.invalidateQueries({ queryKey: ["status"] })
      qc.invalidateQueries({ queryKey: ["videos"] })
      toast.success("Changes saved")
    },
    onError: (e) => toast.error(String(e)),
  })
  const login = useMutation({
    mutationFn: (on: boolean) => api.setAutostart(on),
    onSuccess: (on) => qc.setQueryData(["autostart"], on),
    onError: (e) => toast.error(String(e)),
  })

  const dirty =
    edits !== null &&
    saved !== undefined &&
    JSON.stringify(edits) !== JSON.stringify(saved)
  const set = <K extends keyof Settings>(k: K, v: Settings[K]) =>
    draft && setEdits({ ...draft, [k]: v })

  return (
    <>
      <PageHeader title="Settings" />
      <div className="mx-auto flex w-full max-w-2xl flex-col gap-6 px-4 py-6 md:px-8">
        {!draft ? (
          <Skeleton className="h-96 w-full" />
        ) : (
          <form
            onSubmit={(e) => {
              e.preventDefault()
              save.mutate(draft)
            }}
          >
            <Card>
              <CardHeader>
                <CardTitle>Screening</CardTitle>
                <CardDescription>
                  How tx decides what deserves your time.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <FieldGroup>
                  <Field>
                    <FieldLabel htmlFor="profile">
                      What you want to learn
                    </FieldLabel>
                    <Textarea
                      id="profile"
                      rows={7}
                      placeholder={PROFILE_EXAMPLE}
                      value={draft.profile}
                      onChange={(e) => set("profile", e.target.value)}
                    />
                    <FieldDescription>
                      Every video is scored 0–5 against this. Write it in the
                      language you want analyses in.
                    </FieldDescription>
                  </Field>
                  <Field>
                    <FieldLabel>Notify me from score</FieldLabel>
                    <ToggleGroup
                      type="single"
                      variant="outline"
                      value={String(draft.notify_min)}
                      onValueChange={(v) => v && set("notify_min", Number(v))}
                      aria-label="Minimum score for notifications"
                    >
                      {[0, 1, 2, 3, 4, 5].map((n) => (
                        <ToggleGroupItem
                          key={n}
                          value={String(n)}
                          className="timecode w-10"
                        >
                          {n}
                        </ToggleGroupItem>
                      ))}
                    </ToggleGroup>
                    <FieldDescription>
                      Videos scoring at least this send a desktop notification
                      and appear under Worth your time.
                    </FieldDescription>
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="interval">
                      Check for new videos every
                    </FieldLabel>
                    <InputGroup className="max-w-48">
                      <InputGroupInput
                        id="interval"
                        type="number"
                        min={1}
                        value={draft.interval_min}
                        onChange={(e) =>
                          set("interval_min", Number(e.target.value))
                        }
                      />
                      <InputGroupAddon align="inline-end">
                        <InputGroupText>minutes</InputGroupText>
                      </InputGroupAddon>
                    </InputGroup>
                  </Field>
                  <FieldSeparator />
                  <Field>
                    <FieldLabel htmlFor="model">
                      Transcription for videos without captions
                    </FieldLabel>
                    <Select
                      value={draft.model}
                      onValueChange={(v) => set("model", v)}
                    >
                      <SelectTrigger id="model" className="w-full max-w-72">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectGroup>
                          {(models ?? [draft.model]).map((m) => (
                            <SelectItem key={m} value={m}>
                              {m}{" "}
                              <span className="text-muted-foreground">
                                · {MODEL_NOTES[m] ?? ""}
                              </span>
                            </SelectItem>
                          ))}
                        </SelectGroup>
                      </SelectContent>
                    </Select>
                    <FieldDescription>
                      Runs on this computer. Most YouTube videos have captions
                      and skip this. The model downloads on first use.
                    </FieldDescription>
                  </Field>
                  <Field>
                    <FieldLabel htmlFor="settings-browser">
                      Browser for subscription import
                    </FieldLabel>
                    <Input
                      id="settings-browser"
                      value={draft.browser}
                      onChange={(e) => set("browser", e.target.value)}
                      spellCheck={false}
                    />
                  </Field>
                </FieldGroup>
              </CardContent>
              <CardFooter className="justify-end gap-2">
                <Button
                  type="button"
                  variant="ghost"
                  disabled={!dirty || save.isPending}
                  onClick={() => setEdits(null)}
                >
                  Discard
                </Button>
                <Button type="submit" disabled={!dirty || save.isPending}>
                  {save.isPending && <Spinner data-icon="inline-start" />}
                  Save changes
                </Button>
              </CardFooter>
            </Card>
          </form>
        )}

        <Card>
          <CardHeader>
            <CardTitle>App</CardTitle>
          </CardHeader>
          <CardContent>
            <FieldGroup>
              <Field orientation="horizontal">
                <FieldContent>
                  <FieldLabel htmlFor="autostart">
                    Start tx when I log in
                  </FieldLabel>
                  <FieldDescription>
                    tx checks channels in the background, from the tray. Closing
                    the window keeps it running.
                  </FieldDescription>
                </FieldContent>
                <Switch
                  id="autostart"
                  checked={autostart ?? false}
                  disabled={autostart === undefined || login.isPending}
                  onCheckedChange={(on) => login.mutate(on)}
                />
              </Field>
            </FieldGroup>
          </CardContent>
        </Card>

        <UpdatesCard />
      </div>
    </>
  )
}

/** Version and updates. Installed releases come from GitHub, signed by the release workflow. */
function UpdatesCard() {
  const { data: version } = useAppVersion()
  const { update, install } = useAppUpdate()
  const available = update.data
  const status = update.isFetching
    ? "Checking for updates…"
    : update.isError
      ? `Couldn't check for updates: ${String(update.error)}`
      : available
        ? `tx ${available.version} is available.`
        : update.isSuccess
          ? "You're on the latest version."
          : ""
  return (
    <Card id="updates">
      <CardHeader>
        <CardTitle>Version</CardTitle>
        <CardDescription>
          <span className="timecode text-foreground">tx {version ?? "…"}</span>
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <p className="text-sm text-muted-foreground" aria-live="polite">
          {status}
        </p>
        {available?.body && (
          <p className="text-sm whitespace-pre-line text-muted-foreground">
            {available.body}
          </p>
        )}
        <p className="text-xs text-muted-foreground">
          On Linux .deb and .rpm installs, your system asks for your password to
          install the update.
        </p>
      </CardContent>
      <CardFooter className="justify-end gap-2">
        <Button
          variant="outline"
          disabled={update.isFetching}
          onClick={() => update.refetch()}
        >
          Check for updates
        </Button>
        {available && (
          <Button
            disabled={install.isPending}
            onClick={() => install.mutate(available)}
          >
            {install.isPending && <Spinner data-icon="inline-start" />}
            Update and restart
          </Button>
        )}
      </CardFooter>
    </Card>
  )
}
