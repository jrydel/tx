# Install the tx command on Windows (x64):
#   irm https://raw.githubusercontent.com/jrydel/tx/master/install.ps1 | iex
$ErrorActionPreference = "Stop"

$dir = if ($env:TX_INSTALL_DIR) { $env:TX_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\tx" }
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$exe = Join-Path $dir "tx.exe"
Write-Host "Downloading tx..."
Invoke-WebRequest "https://github.com/jrydel/tx/releases/latest/download/tx-cli-x86_64-pc-windows-msvc.exe" -OutFile $exe -UseBasicParsing

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (($userPath -split ";") -notcontains $dir) {
  [Environment]::SetEnvironmentVariable("Path", "$userPath;$dir", "User")
  Write-Host "Added $dir to your PATH (open a new terminal to use it)."
}
Write-Host "Installed $(& $exe --version) to $exe"
Write-Host ""
# Set up right away in an interactive console; TX_NO_ONBOARD=1 skips it.
if (-not $env:TX_NO_ONBOARD -and [Environment]::UserInteractive -and -not [Console]::IsInputRedirected) {
  & $exe onboard
  if ($LASTEXITCODE -ne 0) { Write-Host "Run '$exe onboard' any time to finish setting up." }
} else {
  Write-Host "Next, set tx up (your AI apps, what you want to learn, what to follow):"
  Write-Host "  $exe onboard"
}
