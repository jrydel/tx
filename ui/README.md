# tx view

React + Tailwind v4 + shadcn/ui source of the MCP App view (the brief and the setup panel).
`bun install && bun run build` writes one self-contained file to `../crates/cli/src/videos.html`, which the Rust binary embeds (`include_str!`); commit that file. `cargo build` never needs bun.

- `src/host.ts`: MCP Apps host bridge (JSON-RPC over postMessage, host theme/size, openai globals).
- `src/components/brief.tsx`: the brief. `src/setup/`: the setup panel and its steps.
- Host theme variables are mapped onto shadcn's tokens in `src/index.css`.
