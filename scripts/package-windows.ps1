<#
.SYNOPSIS
  Builds the Windows installer (NSIS) and the portable zip into release/.
.DESCRIPTION
  1. npm ci (skip with -SkipInstall)  2. tauri build (frontend, budsctl sidecar, app, installer)
  3. zip BudsControl.exe + budsctl.exe + README/LICENSE/notices -> release/BudsControl-<version>-windows-x64-portable.zip
  The installer is copied to release/ too. Nothing is signed (SmartScreen will warn).
#>
param([switch]$SkipInstall)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Run($file, [string[]]$arguments) {
    & $file @arguments
    if ($LASTEXITCODE -ne 0) { throw "$file $($arguments -join ' ') failed with exit code $LASTEXITCODE" }
}

$config = Get-Content (Join-Path $root 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
$version = $config.version
$product = $config.productName

if (-not $SkipInstall) { Run 'npm' @('ci') }
Run 'npm' @('run', 'tauri', 'build', '--', '--config', 'src-tauri/tauri.bundle.conf.json')

$release = Join-Path $root 'release'
New-Item -ItemType Directory -Force $release | Out-Null

# Installer.
$installer = Get-ChildItem (Join-Path $root 'target/release/bundle/nsis') -Filter '*-setup.exe' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $installer) { throw 'the NSIS installer was not produced' }
$installerName = "BudsControl-$version-windows-x64-setup.exe"
Copy-Item $installer.FullName (Join-Path $release $installerName) -Force

# Portable zip: the app, the CLI next to it, and the documents.
$stage = Join-Path $release 'portable'
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $stage | Out-Null
$app = Join-Path $root 'target/release/BudsControl.exe'
if (-not (Test-Path $app)) { $app = Join-Path $root 'target/release/budscontrol.exe' }
Copy-Item $app (Join-Path $stage 'BudsControl.exe')
Copy-Item (Join-Path $root 'target/release/budsctl.exe') (Join-Path $stage 'budsctl.exe')
foreach ($doc in 'README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md') { Copy-Item (Join-Path $root $doc) $stage }
$zip = Join-Path $release "BudsControl-$version-windows-x64-portable.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Remove-Item $stage -Recurse -Force

Get-ChildItem $release -File | ForEach-Object { '{0,-55} {1,8:N1} MB' -f $_.Name, ($_.Length / 1MB) }
