import { useQuery } from "@tanstack/react-query"
import {
  ClapperboardIcon,
  DownloadIcon,
  RefreshCwIcon,
  SlidersHorizontalIcon,
  TvMinimalPlayIcon,
} from "lucide-react"
import { toast } from "sonner"
import { Link, useLocation, useSearch } from "wouter"

import { useAppUpdate, useAppVersion } from "@/hooks/use-app-update"
import { useStatus } from "@/hooks/use-status"
import { Button } from "@/components/ui/button"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import { Spinner } from "@/components/ui/spinner"
import { api } from "@/lib/api"
import { ago } from "@/lib/time"

const NAV = [
  { href: "/", label: "Videos", icon: ClapperboardIcon },
  { href: "/channels", label: "Channels", icon: TvMinimalPlayIcon },
  { href: "/settings", label: "Settings", icon: SlidersHorizontalIcon },
]

/** The tx mark: a frame of film with transcript lines and one grease-pencil cut. */
function Mark() {
  return (
    <svg viewBox="0 0 24 24" className="size-6" aria-hidden>
      <rect
        x="1.5"
        y="4.5"
        width="21"
        height="15"
        rx="2.5"
        fill="var(--foreground)"
      />
      <path
        d="M4 7h2M8 7h2M12 7h2M16 7h2M4 17h2M8 17h2M12 17h2M16 17h2"
        stroke="var(--background)"
        strokeWidth="1.3"
        strokeLinecap="round"
      />
      <path
        d="M5 10.5h7M5 13.5h5"
        stroke="var(--background)"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
      <rect x="15.5" y="9" width="2" height="6" rx="1" fill="var(--marker)" />
    </svg>
  )
}

function StatusLine() {
  const { data } = useStatus()
  if (!data) return null
  const busy = data.now !== "idle"
  const line =
    data.now === "analyzing"
      ? (data.detail ?? "Analyzing")
      : data.now === "checking"
        ? "Checking channels"
        : data.watched
          ? `Checked ${ago(data.checked_at)}`
          : "No channels watched"
  return (
    <div className="flex flex-col gap-2 px-2 pb-1">
      <div
        className="flex min-w-0 items-start gap-2 text-xs text-sidebar-foreground/70"
        aria-live="polite"
      >
        {busy && <Spinner className="mt-px" />}
        <div className="flex min-w-0 flex-col">
          <span className="truncate">{line}</span>
          {data.now === "analyzing" && data.video && (
            <span className="truncate text-sidebar-foreground">
              {data.video}
            </span>
          )}
          {!busy && data.queued > 0 && <span>{data.queued} waiting</span>}
        </div>
      </div>
      <Button
        variant="outline"
        size="sm"
        disabled={busy || !data.watched}
        onClick={() =>
          api.check().then(() => toast("Checking watched channels"))
        }
      >
        <RefreshCwIcon data-icon="inline-start" />
        Check now
      </Button>
    </div>
  )
}

/** Version line, plus an update button when GitHub has a newer release. */
function AppVersion() {
  const { data: version } = useAppVersion()
  const { update, install } = useAppUpdate()
  const available = update.data
  return (
    <div className="flex flex-col gap-2 px-2">
      {available && (
        <Button
          size="sm"
          className="bg-marker text-marker-foreground hover:bg-marker/85"
          disabled={install.isPending}
          onClick={() => install.mutate(available)}
        >
          {install.isPending ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <DownloadIcon data-icon="inline-start" />
          )}
          Update to {available.version}
        </Button>
      )}
      <Link
        href="/settings#updates"
        className="timecode text-[10px] text-sidebar-foreground/50 hover:text-sidebar-foreground"
      >
        tx {version ?? "…"}
      </Link>
    </div>
  )
}

export function AppSidebar() {
  const [location] = useLocation()
  const search = useSearch()
  const active = new URLSearchParams(search).get("channel")
  const { data: channels } = useQuery({
    queryKey: ["channels"],
    queryFn: api.channels,
  })
  const watched = channels?.filter((c) => c.enabled) ?? []

  return (
    <Sidebar>
      <SidebarHeader>
        <Link href="/" className="flex items-center gap-2 px-2 py-1.5">
          <Mark />
          <span className="slate text-xl">tx</span>
        </Link>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {NAV.map(({ href, label, icon: Icon }) => (
                <SidebarMenuItem key={href}>
                  <SidebarMenuButton
                    asChild
                    isActive={
                      href === "/"
                        ? location === "/" && !active
                        : location.startsWith(href)
                    }
                  >
                    <Link href={href}>
                      <Icon />
                      <span>{label}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
        {watched.length > 0 && (
          <SidebarGroup>
            <SidebarGroupLabel>Watching</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {watched.map((c) => (
                  <SidebarMenuItem key={c.id}>
                    <SidebarMenuButton
                      asChild
                      size="sm"
                      isActive={active === c.id}
                    >
                      <Link href={`/?channel=${encodeURIComponent(c.id)}`}>
                        <span className="truncate">{c.title}</span>
                      </Link>
                    </SidebarMenuButton>
                    {c.analyzed > 0 && (
                      <SidebarMenuBadge className="timecode">
                        {c.analyzed}
                      </SidebarMenuBadge>
                    )}
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        )}
      </SidebarContent>
      <SidebarFooter>
        <StatusLine />
        <AppVersion />
      </SidebarFooter>
    </Sidebar>
  )
}
