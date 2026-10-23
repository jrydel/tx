import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"
import { viteSingleFile } from "vite-plugin-singlefile"

// One self-contained HTML file (inline JS + CSS): `scripts/ship.ts` copies it to crates/cli/src/videos.html.
export default defineConfig({
  plugins: [react(), tailwindcss(), viteSingleFile()],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
  build: {
    target: "es2022",
    cssCodeSplit: false,
    chunkSizeWarningLimit: 2000,
  },
})
