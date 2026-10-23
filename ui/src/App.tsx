import { useEffect, useState } from "react"
import { TooltipProvider } from "@/components/ui/tooltip"
import { Brief } from "@/components/brief"
import { Setup } from "@/setup"
import { useHost } from "@/host"
import { isBrief, isOnboarding, type BriefData, type OnboardingData } from "@/types"
import { cn } from "@/lib/utils"

type View = { kind: "brief"; data: BriefData } | { kind: "setup"; data: OnboardingData; key: number }

/** The brief fills the height the app offers (it scrolls inside); the setup panel is at most that tall and never scrolls as a whole. */
export function App() {
  const { output, outputSeq } = useHost()
  const [view, setView] = useState<View | null>(null)

  // Each tool result from the host replaces what's shown; empty ones leave the current view alone.
  useEffect(() => {
    if (isOnboarding(output)) setView({ kind: "setup", data: output, key: outputSeq })
    else if (isBrief(output) && output.videos.length) setView({ kind: "brief", data: output })
  }, [output, outputSeq])

  if (!view) return null
  const single = view.kind === "brief" && view.data.videos.length === 1
  return (
    <TooltipProvider delayDuration={100}>
      <div className={cn("flex flex-col p-0.5", view.kind === "brief" && !single ? "h-(--app-h)" : "max-h-(--app-h)")}>
        {view.kind === "setup" ? (
          <Setup key={view.key} initial={view.data} onBrief={(d) => isBrief(d) && setView({ kind: "brief", data: d })} />
        ) : (
          <Brief data={view.data} />
        )}
      </div>
    </TooltipProvider>
  )
}
