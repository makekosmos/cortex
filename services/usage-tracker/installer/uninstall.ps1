param(
  [string]$InstallDir = "$env:LOCALAPPDATA\Kepler\UsageTracker",
  [switch]$KeepFiles
)

$ErrorActionPreference = "Stop"

$runKeyPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
if (Test-Path $runKeyPath) {
  Remove-ItemProperty -Path $runKeyPath -Name "KeplerUsageTracker" -ErrorAction SilentlyContinue
}

if ((-not $KeepFiles) -and (Test-Path $InstallDir)) {
  Remove-Item -LiteralPath $InstallDir -Recurse -Force
}

Write-Output "Kepler Usage Tracker uninstalled from $InstallDir"
