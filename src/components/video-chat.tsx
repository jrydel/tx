import { useEffect, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  ArrowUpIcon,
  MessagesSquareIcon,
  SquareIcon,
  Trash2Icon,
} from "lucide-react"
import { toast } from "sonner"

import { Markdown } from "@/components/markdown"
import { Bubble, BubbleContent } from "@/components/ui/bubble"
import { Button } from "@/components/ui/button"
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
  InputGroupButton,
  InputGroupText,
  InputGroupTextarea,
} from "@/components/ui/input-group"
import { Message, MessageContent, MessageFooter } from "@/components/ui/message"
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from "@/components/ui/message-scroller"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { api, type ChatEvent, type Message as ChatMessage } from "@/lib/api"

const SUGGESTIONS = [
  "What should I try first, given what I want to learn?",
  "What here is questionable or missing?",
  "Walk me through the part worth skimming.",
]

type Props = {
  videoId: string
  onSeek: (seconds: number) => void
  disabled: boolean
}

/** Follow-up questions about one video, answered by Claude from the transcript, streamed in. */
export function VideoChat({ videoId, onSeek, disabled }: Props) {
  const qc = useQueryClient()
  const key = ["chat", videoId]
  const { data: thread = [] } = useQuery({
    queryKey: key,
    queryFn: () => api.chat(videoId),
  })
  const [draft, setDraft] = useState("")
  const answering = thread.some((m) => m.pending)

  // Stream replies into the cached thread as the engine emits them.
  useEffect(() => {
    const off = listen<ChatEvent>("chat", ({ payload: e }) => {
      if (e.video_id !== videoId) return
      qc.setQueryData<ChatMessage[]>(["chat", videoId], (old) =>
        old?.map((m) =>
          m.id !== e.message_id
            ? m
            : e.kind === "delta"
              ? { ...m, content: m.content + e.text }
              : { ...m, content: e.content, error: e.error, pending: false }
        )
      )
    })
    return () => void off.then((f) => f())
  }, [qc, videoId])

  const send = useMutation({
    mutationFn: (question: string) => api.chatSend(videoId, question),
    onSuccess: (messages) => {
      qc.setQueryData(key, messages)
      setDraft("")
    },
    onError: (e) => toast.error(String(e)),
  })
  const stop = useMutation({
    mutationFn: () => api.chatStop(videoId),
    onError: (e) => toast.error(String(e)),
  })
  const clear = useMutation({
    mutationFn: () => api.chatClear(videoId),
    onSuccess: () => qc.setQueryData(key, []),
    onError: (e) => toast.error(String(e)),
  })
  const ask = (q: string) =>
    q.trim() && !answering && !send.isPending && send.mutate(q.trim())

  return (
    <section
      className="flex min-h-0 flex-1 flex-col rounded-xl border bg-card"
      aria-label="Ask about this video"
    >
      <header className="flex items-center gap-2 border-b px-4 py-2.5">
        <h3 className="slate flex-1 text-lg">Ask about this video</h3>
        {thread.length > 0 && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Clear conversation"
                onClick={() => clear.mutate()}
              >
                <Trash2Icon />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Clear conversation</TooltipContent>
          </Tooltip>
        )}
      </header>

      <MessageScrollerProvider autoScroll defaultScrollPosition="end">
        <MessageScroller className="min-h-0 flex-1">
          <MessageScrollerViewport>
            <MessageScrollerContent className="flex flex-col gap-5 p-4">
              {thread.length === 0 && (
                <MessageScrollerItem messageId="empty">
                  <Empty className="border-0 p-2">
                    <EmptyHeader>
                      <EmptyMedia variant="icon">
                        <MessagesSquareIcon />
                      </EmptyMedia>
                      <EmptyTitle>Ask Claude about this video</EmptyTitle>
                      <EmptyDescription>
                        Answers come from the transcript, with timestamps you
                        can click.
                      </EmptyDescription>
                    </EmptyHeader>
                    <EmptyContent className="items-stretch">
                      {SUGGESTIONS.map((s) => (
                        <Button
                          key={s}
                          variant="outline"
                          size="sm"
                          className="h-auto justify-start py-2 text-left whitespace-normal"
                          disabled={disabled}
                          onClick={() => ask(s)}
                        >
                          {s}
                        </Button>
                      ))}
                    </EmptyContent>
                  </Empty>
                </MessageScrollerItem>
              )}
              {thread.map((m) => (
                <MessageScrollerItem
                  key={m.id}
                  messageId={String(m.id)}
                  scrollAnchor={m.role === "user"}
                >
                  {m.role === "user" ? (
                    <Message align="end">
                      <MessageContent>
                        <Bubble variant="secondary" align="end">
                          <BubbleContent className="whitespace-pre-wrap">
                            {m.content}
                          </BubbleContent>
                        </Bubble>
                      </MessageContent>
                    </Message>
                  ) : (
                    <Message align="start">
                      <MessageContent>
                        <Bubble
                          variant={
                            m.error && m.error !== "Stopped" && !m.content
                              ? "destructive"
                              : "ghost"
                          }
                        >
                          <BubbleContent>
                            {m.content ? (
                              <Markdown text={m.content} onSeek={onSeek} />
                            ) : m.pending ? (
                              <span className="shimmer text-muted-foreground">
                                Reading the transcript…
                              </span>
                            ) : (
                              (m.error ?? "No answer.")
                            )}
                          </BubbleContent>
                        </Bubble>
                        {m.error && (m.content || m.error === "Stopped") && (
                          <MessageFooter>
                            {m.error === "Stopped"
                              ? "Stopped"
                              : `Interrupted: ${m.error}`}
                          </MessageFooter>
                        )}
                      </MessageContent>
                    </Message>
                  )}
                </MessageScrollerItem>
              ))}
            </MessageScrollerContent>
          </MessageScrollerViewport>
          <MessageScrollerButton />
        </MessageScroller>
      </MessageScrollerProvider>

      <form
        className="border-t p-3"
        onSubmit={(e) => {
          e.preventDefault()
          ask(draft)
        }}
      >
        <InputGroup>
          <InputGroupTextarea
            placeholder={
              disabled
                ? "Available once the video is analyzed"
                : "Ask a follow-up question"
            }
            value={draft}
            disabled={disabled}
            rows={1}
            className="max-h-40 min-h-10"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (
                e.key === "Enter" &&
                !e.shiftKey &&
                !e.nativeEvent.isComposing
              ) {
                e.preventDefault()
                ask(draft)
              }
            }}
            aria-label="Follow-up question"
          />
          <InputGroupAddon align="block-end">
            <InputGroupText className="text-xs">
              Enter to send · Shift+Enter for a new line
            </InputGroupText>
            {answering ? (
              <InputGroupButton
                type="button"
                variant="default"
                size="icon-xs"
                className="ml-auto rounded-full"
                aria-label="Stop answering"
                onClick={() => stop.mutate()}
              >
                <SquareIcon className="fill-current" />
              </InputGroupButton>
            ) : (
              <InputGroupButton
                type="submit"
                variant="default"
                size="icon-xs"
                className="ml-auto rounded-full"
                aria-label="Send question"
                disabled={disabled || !draft.trim() || send.isPending}
              >
                <ArrowUpIcon />
              </InputGroupButton>
            )}
          </InputGroupAddon>
        </InputGroup>
      </form>
    </section>
  )
}
