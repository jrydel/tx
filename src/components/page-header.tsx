import { SidebarTrigger } from "@/components/ui/sidebar"
import { Separator } from "@/components/ui/separator"

type Props = {
  title: React.ReactNode
  children?: React.ReactNode
  toolbar?: React.ReactNode
}

export function PageHeader({ title, children, toolbar }: Props) {
  return (
    <header className="sticky top-0 z-10 border-b bg-background/90 backdrop-blur">
      <div className="flex min-h-14 flex-wrap items-center gap-x-3 gap-y-2 px-4 py-2 md:px-8">
        <SidebarTrigger className="-ml-1 md:hidden" />
        <Separator orientation="vertical" className="h-5 md:hidden" />
        <h1 className="slate min-w-0 flex-1 truncate text-2xl">{title}</h1>
        {children}
      </div>
      {toolbar && <div className="px-4 pb-3 md:px-8">{toolbar}</div>}
    </header>
  )
}
