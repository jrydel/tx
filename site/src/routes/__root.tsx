import {
  HeadContent,
  Link,
  Scripts,
  createRootRoute,
} from "@tanstack/react-router"
import { SITE_URL } from "@/lib/site"

import appCss from "../styles.css?url"

const TITLE = "tx: is this video worth watching?"
const DESCRIPTION =
  "Paste a video or podcast link. tx gives you a WATCH, SKIM or SKIP verdict and timestamped takeaways from your own Claude or Codex. Open source, runs on your computer."
const OG_TITLE = "tx: is this video worth watching?"
const OG_IMAGE = `${SITE_URL}/og-image.png`
const OG_ALT =
  "tx video review in a terminal: a SKIM verdict, 4/5 relevance score and a timeline of the part worth watching."

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      { title: TITLE },
      { name: "description", content: DESCRIPTION },
      { name: "color-scheme", content: "light" },
      { name: "theme-color", content: "#ffffff" },
      { property: "og:type", content: "website" },
      { property: "og:site_name", content: "tx" },
      { property: "og:url", content: SITE_URL },
      { property: "og:title", content: OG_TITLE },
      { property: "og:description", content: DESCRIPTION },
      { property: "og:image", content: OG_IMAGE },
      { property: "og:image:width", content: "1200" },
      { property: "og:image:height", content: "630" },
      { property: "og:image:alt", content: OG_ALT },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:title", content: OG_TITLE },
      { name: "twitter:description", content: DESCRIPTION },
      { name: "twitter:image", content: OG_IMAGE },
      { name: "twitter:image:alt", content: OG_ALT },
    ],
    links: [
      { rel: "stylesheet", href: appCss },
      { rel: "canonical", href: SITE_URL },
      { rel: "icon", href: "/favicon.svg", type: "image/svg+xml" },
      { rel: "icon", href: "/favicon.ico", sizes: "48x48" },
      { rel: "apple-touch-icon", href: "/apple-touch-icon.png" },
    ],
  }),
  notFoundComponent: NotFound,
  shellComponent: RootDocument,
})

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  )
}

function NotFound() {
  return (
    <main className="flex min-h-dvh flex-col items-center justify-center gap-4 px-6 text-center">
      <p className="font-mono text-sm text-muted-foreground">404</p>
      <h1 className="text-4xl font-semibold tracking-[-0.03em]">
        Nothing to review here.
      </h1>
      <Link to="/" className="text-sm text-blue hover:underline">
        Back to tx
      </Link>
    </main>
  )
}
