param(
  [string]$InstallDir = "$env:LOCALAPPDATA\Kosmos\Kepler",
  [switch]$KeepFiles,
  [switch]$KeepDb
)

$ErrorActionPreference = "Stop"

# 1. Убрать HKCU Run autostart.
$runKeyPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
if (Test-Path $runKeyPath) {
  Remove-ItemProperty -Path $runKeyPath -Name "KosmosKepler" -ErrorAction SilentlyContinue
}

# 2. Остановить запущенные процессы (Kepler + watcher).
foreach ($name in @("kepler-watcher", "kepler")) {
  Get-Process -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
    Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
  }
}

# 3. Удалить lock-файл (на случай если процесс не успел graceful cleanup).
$lockFile = Join-Path $env:APPDATA "Kosmos\kepler.lock.json"
if (Test-Path $lockFile) {
  Remove-Item -LiteralPath $lockFile -Force -ErrorAction SilentlyContinue
}
$singletonLock = Join-Path $env:APPDATA "Kosmos\kepler-singleton.lock.db"
if (Test-Path $singletonLock) {
  Remove-Item -LiteralPath $singletonLock -Force -ErrorAction SilentlyContinue
}

# 4. Удалить файлы установки (бинари + иконка).
if ((-not $KeepFiles) -and (Test-Path $InstallDir)) {
  Remove-Item -LiteralPath $InstallDir -Recurse -Force
}

# 5. Опционально удалить ARK DB.
if (-not $KeepDb) {
  $arkDb = Join-Path $env:APPDATA "Kosmos\ark.db"
  if (Test-Path $arkDb) {
    Remove-Item -LiteralPath $arkDb -Force -ErrorAction SilentlyContinue
  }
}

Write-Output "Kosmos Kepler uninstalled from $InstallDir"
