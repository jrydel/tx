import { CodeBlock } from "@/components/code-block"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { INSTALL } from "@/lib/site"

/** Per-OS install commands, exactly as in the README. One line each; long commands scroll. */
export function InstallTabs({ className }: { className?: string }) {
  return (
    <Tabs defaultValue="unix" className={className}>
      <TabsList aria-label="Operating system" className="self-center">
        <TabsTrigger value="unix" className="px-3 font-mono text-xs">
          macOS · Linux
        </TabsTrigger>
        <TabsTrigger value="windows" className="px-3 font-mono text-xs">
          Windows
        </TabsTrigger>
      </TabsList>
      <TabsContent value="unix">
        <CodeBlock
          code={INSTALL.unix}
          className="text-[11.5px] sm:text-[12px]"
        />
      </TabsContent>
      <TabsContent value="windows">
        <CodeBlock
          code={INSTALL.windows}
          prompt="PS>"
          className="text-[11.5px] sm:text-[12px]"
        />
      </TabsContent>
    </Tabs>
  )
}
