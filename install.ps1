#Requires -Version 5.1
<#
.SYNOPSIS
    mdview installer for Windows — downloads a prebuilt binary and helps wire up Claude Code.
.DESCRIPTION
    irm https://raw.githubusercontent.com/vantt/mdview/main/install.ps1 | iex
.NOTES
    Env overrides (set before running):
      $env:MDVIEW_INSTALL_DIR   target dir (default: $env:USERPROFILE\.mdview\bin)
      $env:MDVIEW_VERSION       release tag (default: latest)
#>

$ErrorActionPreference = 'Stop'

$Repo = 'vantt/mdview'
$Bin = 'mdview'
$Target = 'x86_64-pc-windows-msvc'

function Write-Info($msg) { Write-Host "  $msg" }
function Write-ErrExit($msg) { Write-Error "error: $msg"; exit 1 }

if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -and $env:PROCESSOR_ARCHITEW6432 -ne 'AMD64') {
    Write-ErrExit "unsupported architecture: $env:PROCESSOR_ARCHITECTURE (only x86_64/AMD64 prebuilt binaries are published; build from source: cargo install --git https://github.com/$Repo mdview)"
}

$InstallDir = if ($env:MDVIEW_INSTALL_DIR) { $env:MDVIEW_INSTALL_DIR } else { Join-Path $env:USERPROFILE '.mdview\bin' }
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$Version = if ($env:MDVIEW_VERSION) { $env:MDVIEW_VERSION } else { 'latest' }
$AssetName = "$Bin-$Target.exe"
$Url = if ($Version -eq 'latest') {
    "https://github.com/$Repo/releases/latest/download/$AssetName"
} else {
    "https://github.com/$Repo/releases/download/$Version/$AssetName"
}

Write-Host "Installing mdview..."
Write-Info "target: $Target"
Write-Info "into:   $InstallDir"

$DestExe = Join-Path $InstallDir "$Bin.exe"
try {
    Invoke-WebRequest -Uri $Url -OutFile $DestExe -UseBasicParsing
    Write-Info "downloaded prebuilt binary"
} catch {
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Info "no prebuilt release found; building from source with cargo..."
        cargo install --git "https://github.com/$Repo" $Bin --root $InstallDir
    } else {
        Write-ErrExit "no prebuilt binary for $Target and cargo not found. Install Rust, then: cargo install --git https://github.com/$Repo $Bin"
    }
}

$pathDirs = $env:Path -split ';'
if ($pathDirs -notcontains $InstallDir) {
    Write-Info "NOTE: $InstallDir is not on your PATH. Add it, e.g.:"
    Write-Info "  [Environment]::SetEnvironmentVariable('Path', `"`$env:Path;$InstallDir`", 'User')"
    Write-Info "  (then restart your terminal)"
}

Write-Host ""
if (Test-Path $DestExe) {
    $ver = & $DestExe --version
    Write-Host "Installed $ver."
} else {
    Write-Host "Installed."
}
Write-Host "Next:"
Write-Host "  $Bin doctor --fix     # wire up Claude Code MCP integration"
Write-Host "  $Bin serve            # start the viewer (http://localhost:7700)"
