import { SidebarTrigger } from "@/components/ui/sidebar"
import { Separator } from "@/components/ui/separator"

export function PageHeader({
  title,
  children,
}: {
  title: React.ReactNode
  children?: React.ReactNode
}) {
  return (
    <header className="sticky top-0 z-10 flex min-h-14 flex-wrap items-center gap-x-3 gap-y-2 border-b bg-background/90 px-4 py-2 backdrop-blur md:px-8">
      <SidebarTrigger className="-ml-1 md:hidden" />
      <Separator orientation="vertical" className="h-5 md:hidden" />
      <h1 className="slate min-w-0 flex-1 truncate text-2xl">{title}</h1>
      {children}
    </header>
  )
}
