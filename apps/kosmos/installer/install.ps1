param(
  [string]$InstallDir = "$env:LOCALAPPDATA\Kepler\Kosmos",
  [switch]$NoStartup,
  [switch]$NoLaunch
)

$ErrorActionPreference = "Stop"

$sourceRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$sourceKosmos = Join-Path $sourceRoot "kosmos.exe"
$sourceArkRpc = Join-Path $sourceRoot "ark-core-rpc.exe"
$sourceWatcher = Join-Path $sourceRoot "kosmos-watcher.exe"
$sourceIcon = Join-Path $sourceRoot "kosmos.png"

if (-not (Test-Path $sourceKosmos)) {
  throw "kosmos.exe was not found next to install.ps1 (expected at $sourceKosmos)"
}
if (-not (Test-Path $sourceArkRpc)) {
  throw "ark-core-rpc.exe was not found next to install.ps1 (expected at $sourceArkRpc)"
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$targetKosmos = Join-Path $InstallDir "kosmos.exe"
$targetArkRpc = Join-Path $InstallDir "ark-core-rpc.exe"
Copy-Item -LiteralPath $sourceKosmos -Destination $targetKosmos -Force
Copy-Item -LiteralPath $sourceArkRpc -Destination $targetArkRpc -Force

if (Test-Path $sourceWatcher) {
  $targetWatcher = Join-Path $InstallDir "kosmos-watcher.exe"
  Copy-Item -LiteralPath $sourceWatcher -Destination $targetWatcher -Force
}

if (Test-Path $sourceIcon) {
  $targetIcon = Join-Path $InstallDir "kosmos.png"
  Copy-Item -LiteralPath $sourceIcon -Destination $targetIcon -Force
}

# HKCU Run autostart. Если есть watcher — он управляет жизненным циклом kosmos.exe
# (respawn при крахе); иначе автостартуем kosmos напрямую.
if (-not $NoStartup) {
  $runKeyPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
  New-Item -Path $runKeyPath -Force | Out-Null
  $autostartTarget = if (Test-Path (Join-Path $InstallDir "kosmos-watcher.exe")) {
    Join-Path $InstallDir "kosmos-watcher.exe"
  } else {
    $targetKosmos
  }
  Set-ItemProperty -Path $runKeyPath -Name "KeplerKosmos" -Value "`"$autostartTarget`""
}

# Готовим Kepler dir в APPDATA — Kosmos сам apply'нёт ACL для lock-file через icacls
# (см. apps/kosmos/src/lock_file.rs::apply_owner_only_permissions).
$keplerAppData = Join-Path $env:APPDATA "Kepler"
if (-not (Test-Path $keplerAppData)) {
  New-Item -ItemType Directory -Force -Path $keplerAppData | Out-Null
}

if (-not $NoLaunch) {
  $launchTarget = if (Test-Path (Join-Path $InstallDir "kosmos-watcher.exe")) {
    Join-Path $InstallDir "kosmos-watcher.exe"
  } else {
    $targetKosmos
  }
  Start-Process -FilePath $launchTarget -WindowStyle Hidden
}

Write-Output "Kepler Kosmos installed to $InstallDir"
if (-not (Test-Path (Join-Path $InstallDir "kosmos-watcher.exe"))) {
  Write-Warning "kosmos-watcher.exe not found - autostart launches kosmos.exe directly without crash recovery."
}
