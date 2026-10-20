// Puts the tools tx bundles into src-tauri/binaries/<name>-<target-triple>[.exe], the layout Tauri's
// `externalBin` expects: yt-dlp (official standalone build), QuickJS-ng (yt-dlp's JS runtime for
// YouTube), and the `tx` CLI built from this repo.
//
//   bun scripts/sidecars.ts [--target <triple>] [--skip-cli]

import { $ } from "bun"
import { chmod, mkdir } from "node:fs/promises"

const YT_DLP = "2026.08.19" // first install only; tx keeps its own copy current at runtime
const QUICKJS = "v0.17.0"

const DOWNLOADS: Record<string, { ytdlp: string; qjs: string }> = {
  "x86_64-unknown-linux-gnu": {
    ytdlp: "yt-dlp_linux",
    qjs: "qjs-linux-x86_64",
  },
  "aarch64-unknown-linux-gnu": {
    ytdlp: "yt-dlp_linux_aarch64",
    qjs: "qjs-linux-aarch64",
  },
  "x86_64-apple-darwin": { ytdlp: "yt-dlp_macos", qjs: "qjs-darwin-x86_64" },
  "aarch64-apple-darwin": { ytdlp: "yt-dlp_macos", qjs: "qjs-darwin-arm64" },
  "x86_64-pc-windows-msvc": {
    ytdlp: "yt-dlp.exe",
    qjs: "qjs-windows-x86_64.exe",
  },
}

const args = process.argv.slice(2)
const flag = (name: string) => args[args.indexOf(name) + 1]
const host = (await $`rustc -vV`.text()).match(/host: (\S+)/)![1]
const target = args.includes("--target")
  ? flag("--target")
  : (process.env.TAURI_ENV_TARGET_TRIPLE ?? host)
const assets = DOWNLOADS[target]
if (!assets) throw new Error(`no sidecar downloads for target ${target}`)
const exe = target.includes("windows") ? ".exe" : ""
const dir = new URL("../src-tauri/binaries/", import.meta.url).pathname
await mkdir(dir, { recursive: true })

async function download(url: string, name: string) {
  const dest = `${dir}${name}-${target}${exe}`
  if (await Bun.file(dest).exists()) return console.log(`have ${name}`)
  console.log(`download ${url}`)
  const res = await fetch(url, { redirect: "follow" })
  if (!res.ok) throw new Error(`${url}: ${res.status}`)
  await Bun.write(dest, res)
  await chmod(dest, 0o755)
}

await download(
  `https://github.com/yt-dlp/yt-dlp/releases/download/${YT_DLP}/${assets.ytdlp}`,
  "yt-dlp"
)
await download(
  `https://github.com/quickjs-ng/quickjs/releases/download/${QUICKJS}/${assets.qjs}`,
  "qjs"
)

if (!args.includes("--skip-cli")) {
  const root = new URL("../src-tauri/", import.meta.url).pathname
  // Same features as the app build (e.g. --features metal), so tx-core compiles once.
  const features = args.includes("--features")
    ? ["--features", flag("--features")]
    : []
  await $`cargo build --release -p tx-cli --target ${target} ${features}`.cwd(
    root
  )
  await Bun.write(
    `${dir}tx-${target}${exe}`,
    Bun.file(`${root}target/${target}/release/tx${exe}`)
  )
  await chmod(`${dir}tx-${target}${exe}`, 0o755)
  console.log("built tx CLI")
}
