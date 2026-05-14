param(
  [string]$InstallDir = "$env:LOCALAPPDATA\Kosmos\Kepler",
  [switch]$NoStartup,
  [switch]$NoLaunch
)

$ErrorActionPreference = "Stop"

$sourceRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$sourceKepler = Join-Path $sourceRoot "kepler.exe"
$sourceArkRpc = Join-Path $sourceRoot "ark-core-rpc.exe"
$sourceWatcher = Join-Path $sourceRoot "kepler-watcher.exe"
$sourceIcon = Join-Path $sourceRoot "kepler.png"

if (-not (Test-Path $sourceKepler)) {
  throw "kepler.exe was not found next to install.ps1 (expected at $sourceKepler)"
}
if (-not (Test-Path $sourceArkRpc)) {
  throw "ark-core-rpc.exe was not found next to install.ps1 (expected at $sourceArkRpc)"
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$targetKepler = Join-Path $InstallDir "kepler.exe"
$targetArkRpc = Join-Path $InstallDir "ark-core-rpc.exe"
Copy-Item -LiteralPath $sourceKepler -Destination $targetKepler -Force
Copy-Item -LiteralPath $sourceArkRpc -Destination $targetArkRpc -Force

if (Test-Path $sourceWatcher) {
  $targetWatcher = Join-Path $InstallDir "kepler-watcher.exe"
  Copy-Item -LiteralPath $sourceWatcher -Destination $targetWatcher -Force
}

if (Test-Path $sourceIcon) {
  $targetIcon = Join-Path $InstallDir "kepler.png"
  Copy-Item -LiteralPath $sourceIcon -Destination $targetIcon -Force
}

# HKCU Run autostart. Если есть watcher — он управляет жизненным циклом kepler.exe
# (respawn при крахе); иначе автостартуем kepler напрямую.
if (-not $NoStartup) {
  $runKeyPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
  New-Item -Path $runKeyPath -Force | Out-Null
  $autostartTarget = if (Test-Path (Join-Path $InstallDir "kepler-watcher.exe")) {
    Join-Path $InstallDir "kepler-watcher.exe"
  } else {
    $targetKepler
  }
  Set-ItemProperty -Path $runKeyPath -Name "KosmosKepler" -Value "`"$autostartTarget`""
}

# Готовим Kosmos dir в APPDATA — Kepler сам apply'нёт ACL для lock-file через icacls
# (см. apps/kepler/src/lock_file.rs::apply_owner_only_permissions).
$kosmosAppData = Join-Path $env:APPDATA "Kosmos"
if (-not (Test-Path $kosmosAppData)) {
  New-Item -ItemType Directory -Force -Path $kosmosAppData | Out-Null
}

if (-not $NoLaunch) {
  $launchTarget = if (Test-Path (Join-Path $InstallDir "kepler-watcher.exe")) {
    Join-Path $InstallDir "kepler-watcher.exe"
  } else {
    $targetKepler
  }
  Start-Process -FilePath $launchTarget -WindowStyle Hidden
}

Write-Output "Kosmos Kepler installed to $InstallDir"
if (-not (Test-Path (Join-Path $InstallDir "kepler-watcher.exe"))) {
  Write-Warning "kepler-watcher.exe not found - autostart launches kepler.exe directly without crash recovery."
}
