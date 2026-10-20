import { useEffect, useRef } from "react"
import { listen } from "@tauri-apps/api/event"
import { useQueryClient } from "@tanstack/react-query"
import { Route, Router, Switch, useLocation } from "wouter"
import { useHashLocation } from "wouter/use-hash-location"

import { AppSidebar } from "@/components/app-sidebar"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { useStatus } from "@/hooks/use-status"
import { ChannelsPage } from "@/pages/channels"
import { SettingsPage } from "@/pages/settings"
import { VideoPage } from "@/pages/video"
import { VideosPage } from "@/pages/videos"

/** Refresh lists when the worker finishes something; follow notification clicks; start each page at the top. */
function EngineSync() {
  const qc = useQueryClient()
  const [path, navigate] = useLocation()
  const { data } = useStatus()
  const key = `${data?.now}|${data?.video}|${data?.queued}|${data?.checked_at}`
  const prev = useRef(key)
  useEffect(() => {
    if (prev.current !== key) {
      prev.current = key
      qc.invalidateQueries({ queryKey: ["videos"] })
      qc.invalidateQueries({ queryKey: ["video"] })
      qc.invalidateQueries({ queryKey: ["channels"] })
    }
  }, [key, qc])
  useEffect(() => {
    const off = listen<string>("navigate", (e) => navigate(e.payload))
    return () => void off.then((f) => f())
  }, [navigate])
  useEffect(() => {
    window.scrollTo(0, 0) // block body: scrollTo returns a Promise in newer engines, not a cleanup
  }, [path])
  return null
}

export default function App() {
  return (
    <Router hook={useHashLocation}>
      <EngineSync />
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset>
          <Switch>
            <Route path="/" component={VideosPage} />
            <Route path="/v/:id" component={VideoPage} />
            <Route path="/channels" component={ChannelsPage} />
            <Route path="/settings" component={SettingsPage} />
            <Route component={VideosPage} />
          </Switch>
        </SidebarInset>
      </SidebarProvider>
    </Router>
  )
}
