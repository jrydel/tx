import { cn } from "@/lib/utils"

/**
 * tx's real output, rebuilt as HTML from .github/assets/tx-tutorial.jpg
 * ("REAL tx 0.20.0 OUTPUT · example profile"). Text is copied from that image.
 */
export function ReviewTerminal({ className }: { className?: string }) {
  return (
    <figure
      className={cn(
        "term overflow-hidden rounded-2xl font-mono text-[12.5px] leading-[1.65] shadow-(--shadow-card) sm:text-[13px]",
        className
      )}
    >
      <div
        className="flex items-center gap-2 px-4 py-3"
        style={{ background: "var(--t-bar)" }}
      >
        <span className="size-2.5 rounded-full bg-[#ff5f57]" />
        <span className="size-2.5 rounded-full bg-[#febc2e]" />
        <span className="size-2.5 rounded-full bg-[#28c840]" />
        <span
          className="flex-1 pr-12 text-center text-xs"
          style={{ color: "var(--t-grey)" }}
        >
          Terminal
        </span>
      </div>

      <div className="px-4 pt-4 pb-5 sm:px-5">
        <p className="mb-3 text-[11px]" style={{ color: "var(--t-grey)" }}>
          REAL tx 0.20.0 OUTPUT · example profile
        </p>

        <div className="flex items-center gap-2">
          <span aria-hidden="true" style={{ color: "var(--t-grey)" }}>
            ──
          </span>
          <b>tx</b>
          <span style={{ color: "var(--t-grey)" }}>· video review</span>
          <span
            aria-hidden="true"
            className="h-px flex-1"
            style={{ background: "#4a4f51" }}
          />
        </div>

        <div className="mt-4 pl-1 sm:pl-3">
          <p className="font-bold">
            Did a 50 year old military secret just solve agent prompt injection?
          </p>
          <p style={{ color: "var(--t-grey)" }}>Fireship · 05:03</p>

          <p className="mt-4 flex items-center gap-3">
            <span
              className="px-2 font-bold"
              style={{ background: "var(--t-amber)", color: "#1a1405" }}
            >
              SKIM
            </span>
            <span style={{ color: "var(--t-green)" }}>
              <span aria-hidden="true">●●●●○</span> 4/5
            </span>
          </p>
          <p>
            Worth a skim. The one useful idea is applying information flow
            control (taint labels) to agent sessions as a deterministic guard
            against prompt injection, but Open Appa sponsors this video, so its
            claims and benchmark numbers come from the sponsor and nobody has
            verified them.
          </p>

          <p className="mt-4 font-bold">Timeline</p>
          <div
            role="img"
            aria-label="Timeline of 00:00 to 05:03; the worth-watching part runs 01:47 to 04:44"
            className="mt-1 flex h-3.5 w-full"
          >
            <span className="term-bar-hatch" style={{ flex: "107 0 0" }} />
            <span className="term-bar-fill" style={{ flex: "177 0 0" }} />
            <span className="term-bar-hatch" style={{ flex: "19 0 0" }} />
          </div>
          <p
            className="mt-1 flex justify-between"
            style={{ color: "var(--t-cyan)" }}
          >
            <span>00:00</span>
            <span>05:03</span>
          </p>

          <p className="mt-4">
            <b style={{ color: "var(--t-amber)" }}>skim</b>
            <span style={{ color: "var(--t-grey)" }}> · </span>
            <span style={{ color: "var(--t-cyan)" }}>01:47-04:44</span>
          </p>
          <p>
            This part covers why current defenses fail, how the session
            classification model works, and the demo with its benchmark. The
            opening Australia and Nvidia segment is unverified news framing, and
            the outro is sponsor promotion.
          </p>
        </div>
      </div>
    </figure>
  )
}
