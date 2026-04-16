param(
  [string]$OutputDir = "dist\KeplerUsageTrackerInstaller",
  [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..")
$serviceRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$targetDir = Join-Path $serviceRoot "target\release"
$bundleDir = Join-Path $serviceRoot $OutputDir
$zipPath = "${bundleDir}.zip"

if (-not $SkipBuild) {
  cargo build --release --manifest-path (Join-Path $serviceRoot "Cargo.toml")
}

$exePath = Join-Path $targetDir "usage-tracker.exe"
if (-not (Test-Path $exePath)) {
  throw "Release binary not found at $exePath"
}

if (Test-Path $bundleDir) {
  Remove-Item -LiteralPath $bundleDir -Recurse -Force
}
if (Test-Path $zipPath) {
  Remove-Item -LiteralPath $zipPath -Force
}

New-Item -ItemType Directory -Force -Path $bundleDir | Out-Null

Copy-Item -LiteralPath $exePath -Destination (Join-Path $bundleDir "usage-tracker.exe")
Copy-Item -LiteralPath (Join-Path $serviceRoot "installer\install.ps1") -Destination (Join-Path $bundleDir "install.ps1")
Copy-Item -LiteralPath (Join-Path $serviceRoot "installer\uninstall.ps1") -Destination (Join-Path $bundleDir "uninstall.ps1")
Copy-Item -LiteralPath (Join-Path $serviceRoot "README.md") -Destination (Join-Path $bundleDir "README.md")

$installCmd = @'
@echo off
powershell -ExecutionPolicy Bypass -File "%~dp0install.ps1" %*
'@
$uninstallCmd = @'
@echo off
powershell -ExecutionPolicy Bypass -File "%~dp0uninstall.ps1" %*
'@

Set-Content -LiteralPath (Join-Path $bundleDir "Install Usage Tracker.cmd") -Value $installCmd -Encoding ASCII
Set-Content -LiteralPath (Join-Path $bundleDir "Uninstall Usage Tracker.cmd") -Value $uninstallCmd -Encoding ASCII

$manifest = @{
  product = "Kepler Usage Tracker"
  version = "0.1.0"
  built_at_utc = (Get-Date).ToUniversalTime().ToString("o")
  install_dir_default = "%LOCALAPPDATA%\Kepler\UsageTracker"
  entrypoint = "usage-tracker.exe"
  install_command = "Install Usage Tracker.cmd"
  uninstall_command = "Uninstall Usage Tracker.cmd"
} | ConvertTo-Json

Set-Content -LiteralPath (Join-Path $bundleDir "manifest.json") -Value $manifest -Encoding UTF8

Compress-Archive -Path (Join-Path $bundleDir "*") -DestinationPath $zipPath -Force

Write-Output "Installer bundle ready: $bundleDir"
Write-Output "Installer zip ready: $zipPath"
