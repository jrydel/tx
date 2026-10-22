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
Write-Host "Next, open a new terminal and run 'tx' to set it up (your AI apps, what you want to learn, what to follow)."
