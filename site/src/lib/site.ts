/**
 * Facts shown on the page. Each one comes from the tx repo:
 * README.md, install.sh, install.ps1, Cargo.toml (`workspace.package.version`),
 * LICENSE and the latest GitHub release.
 */
export const SITE_URL = "https://tx.todayworks.ai"
export const REPO_URL = "https://github.com/jrydel/tx"
export const RELEASES_URL = `${REPO_URL}/releases`
export const LICENSE_URL = `${REPO_URL}/blob/master/LICENSE`
export const TUTORIAL_URL = `${REPO_URL}/releases/download/v0.20.0/tx-terminal.mp4`
export const STUDIO_URL = "https://todayworks.ai"
export const AUTHOR_URL = "https://github.com/jrydel"
export const VERSION = "0.20.0"

export const INSTALL = {
  unix: `curl -fsSL https://raw.githubusercontent.com/jrydel/tx/master/install.sh | sh`,
  windows: `irm https://raw.githubusercontent.com/jrydel/tx/master/install.ps1 | iex`,
} as const

export const EXAMPLE_URL = "https://www.youtube.com/watch?v=USeyDcWyVCc"
