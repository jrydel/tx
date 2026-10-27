# tx site

Landing page for tx at https://tx.todayworks.ai. TanStack Start on Cloudflare Workers, shadcn/ui (Base UI), Tailwind v4, bun.

```sh
bun install
bun run dev        # local dev server
bun run build
bun run typecheck && bun run lint && bun run check
bun run deploy     # vite build && wrangler deploy (Worker "tx-site", custom domain tx.todayworks.ai)
```

Every claim on the page comes from the repo root: `README.md`, `install.sh`, `install.ps1`, `LICENSE`, `Cargo.toml` (version) and the latest GitHub release. `src/lib/site.ts` holds the version and install commands; update it when tx ships a new release. The terminal block in `src/components/terminal.tsx` recreates the output in `.github/assets/tx-tutorial.jpg`.

`public/og-image.png` is rendered from `og/og.html` (1200x630): serve this folder and screenshot that page at 1200x630.
