import {
  HeadContent,
  Link,
  Scripts,
  createRootRoute,
} from "@tanstack/react-router"
import { THEME_SCRIPT } from "@/components/theme-toggle"
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
      { name: "color-scheme", content: "dark light" },
      {
        name: "theme-color",
        content: "#0a0a0a",
        media: "(prefers-color-scheme: dark)",
      },
      {
        name: "theme-color",
        content: "#ffffff",
        media: "(prefers-color-scheme: light)",
      },
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
    scripts: [{ children: THEME_SCRIPT }],
  }),
  notFoundComponent: NotFound,
  shellComponent: RootDocument,
})

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    // The inline theme script may swap `dark` for light before hydration.
    <html lang="en" className="dark" suppressHydrationWarning>
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
    <main className="wrap flex min-h-svh flex-col justify-center gap-4">
      <p className="eyebrow">404</p>
      <h1 className="h1">Nothing to review here.</h1>
      <Link to="/" className="font-mono text-sm underline underline-offset-4">
        Back to tx
      </Link>
    </main>
  )
}
