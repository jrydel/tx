// Copies the built single file to where the Rust binary embeds it from.
import { copyFileSync, statSync } from "node:fs"
import { resolve } from "node:path"

const from = resolve(import.meta.dirname, "../dist/index.html")
const to = resolve(import.meta.dirname, "../../crates/cli/src/videos.html")
copyFileSync(from, to)
console.log(`videos.html: ${(statSync(to).size / 1024).toFixed(1)} KiB`)
