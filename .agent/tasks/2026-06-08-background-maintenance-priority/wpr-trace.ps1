# Phase 3 verification — WPR A/B trace для background-maintenance priority.
#
# Снимает Windows Performance Recorder trace (CPU + Disk I/O + File I/O) во время
# cold start Kosmos, чтобы цифрами подтвердить, что app_index/file_index/db_backup
# идут на background-priority потоках и НЕ насыщают CPU/диск (USB не проседает).
#
# Запускать ОТ АДМИНИСТРАТОРА (wpr требует elevation). Снять дважды:
#   1) BEFORE — на коммите до d94468b8^ (или revert 4 коммитов фикса);
#   2) AFTER  — на текущем main.
# Затем сравнить в WPA (Windows Performance Analyzer).
#
# Usage (elevated PowerShell):
#   .\.agent\tasks\2026-06-08-background-maintenance-priority\wpr-trace.ps1 -Label after

param(
    [string]$Label = "after"
)

$ErrorActionPreference = "Stop"

# --- elevation check -------------------------------------------------------
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()
).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Error "wpr требует прав администратора. Запусти PowerShell от имени администратора."
    exit 1
}

if (-not (Get-Command wpr -ErrorAction SilentlyContinue)) {
    Write-Error "wpr не найден. Установи Windows Performance Toolkit (часть Windows ADK)."
    exit 1
}

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$outDir = Join-Path $PSScriptRoot "perf"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$etl = Join-Path $outDir "kosmos-$Label-$stamp.etl"

# Сброс возможной зависшей сессии.
wpr -cancel 2>$null | Out-Null

Write-Host "[wpr] starting CPU + DiskIO + FileIO recording..." -ForegroundColor Cyan
wpr -start CPU -start DiskIO -start FileIO -filemode

Write-Host ""
Write-Host "Воспроизведи нагрузку (это и есть измеряемый сценарий):" -ForegroundColor Yellow
Write-Host "  1. Сделай COLD START Kosmos (убедись, что kepler-backend не запущен)."
Write-Host "  2. Открой launcher по хоткею, поскролль список приложений."
Write-Host "  3. Подожди ~20-25с (app_index initial rescan стартует через 15с)."
Write-Host "  4. (опц.) форсируй backup: запусти backend с"
Write-Host "     KEPLER_BACKUP_INTERVAL_HOURS=0 KEPLER_BACKUP_DELAY_MS=2000"
Write-Host "  5. Параллельно подвигай окно / попользуй USB-устройство — проверь лаги."
Write-Host ""
Read-Host "Когда сценарий отыгран — нажми Enter чтобы остановить запись"

Write-Host "[wpr] stopping → $etl" -ForegroundColor Cyan
wpr -stop $etl "Kosmos background-maintenance priority ($Label)"

Write-Host ""
Write-Host "Готово: $etl" -ForegroundColor Green
Write-Host ""
Write-Host "Анализ в WPA (wpa.exe `"$etl`"):" -ForegroundColor Yellow
Write-Host "  CPU Usage (Sampled) → по процессам: kepler-backend.exe, ark-core-rpc.exe,"
Write-Host "    Electron, dwm.exe, System/Interrupts/DPC — есть ли normal-priority burst."
Write-Host "  Thread Activity → у ark-db-backup / app_index blocking-потоков приоритет"
Write-Host "    должен быть пониженным (background)."
Write-Host "  Disk Usage / File I/O → по файлам: app-index.db, app-icons\*, ark.db.backup-*,"
Write-Host "    file-index.db — I/O-приоритет должен быть Low/Very Low, без burst в момент cold start."
Write-Host ""
Write-Host "Сравни BEFORE vs AFTER: AFTER должен показать сдвиг тяжёлого I/O в background-приоритет"
Write-Host "и отсутствие normal-priority CPU/disk spike в первые секунды старта."
