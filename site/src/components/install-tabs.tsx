import { CodeBlock } from "@/components/code-block"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { INSTALL } from "@/lib/site"

/** Per-OS install commands, exactly as in the README. */
export function InstallTabs({ className }: { className?: string }) {
  return (
    <Tabs defaultValue="unix" className={className}>
      <TabsList aria-label="Operating system">
        <TabsTrigger value="unix" className="px-3 font-mono text-xs">
          macOS · Linux
        </TabsTrigger>
        <TabsTrigger value="windows" className="px-3 font-mono text-xs">
          Windows
        </TabsTrigger>
      </TabsList>
      <TabsContent value="unix" className="flex flex-col gap-2">
        <CodeBlock code={INSTALL.unix} wrap />
        <p className="font-mono text-[11px] text-dim">
          macOS (Apple Silicon) or Linux (x86_64)
        </p>
      </TabsContent>
      <TabsContent value="windows" className="flex flex-col gap-2">
        <CodeBlock code={INSTALL.windows} prompt="PS>" wrap />
        <p className="font-mono text-[11px] text-dim">
          Windows (x64), in PowerShell
        </p>
      </TabsContent>
    </Tabs>
  )
}
