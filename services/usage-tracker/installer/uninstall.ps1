param(
  [string]$InstallDir = "$env:LOCALAPPDATA\Kosmos\UsageTracker",
  [switch]$KeepFiles
)

$ErrorActionPreference = "Stop"

$runKeyPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
if (Test-Path $runKeyPath) {
  Remove-ItemProperty -Path $runKeyPath -Name "KosmosUsageTracker" -ErrorAction SilentlyContinue
}

if ((-not $KeepFiles) -and (Test-Path $InstallDir)) {
  Remove-Item -LiteralPath $InstallDir -Recurse -Force
}

Write-Output "Kosmos Usage Tracker uninstalled from $InstallDir"
