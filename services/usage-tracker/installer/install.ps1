param(
  [string]$InstallDir = "$env:LOCALAPPDATA\Kepler\UsageTracker",
  [switch]$NoStartup,
  [switch]$NoLaunch
)

$ErrorActionPreference = "Stop"

$sourceRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$sourceExe = Join-Path $sourceRoot "usage-tracker.exe"

if (-not (Test-Path $sourceExe)) {
  throw "usage-tracker.exe was not found next to install.ps1"
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$targetExe = Join-Path $InstallDir "usage-tracker.exe"
Copy-Item -LiteralPath $sourceExe -Destination $targetExe -Force

if (-not $NoStartup) {
  $runKeyPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
  New-Item -Path $runKeyPath -Force | Out-Null
  Set-ItemProperty -Path $runKeyPath -Name "KeplerUsageTracker" -Value "`"$targetExe`""
}

if (-not $NoLaunch) {
  Start-Process -FilePath $targetExe -WindowStyle Hidden
}

Write-Output "Kepler Usage Tracker installed to $InstallDir"
