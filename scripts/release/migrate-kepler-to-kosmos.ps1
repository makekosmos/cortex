<#
.SYNOPSIS
    Миграция пользовательских данных после rebrand: Kepler (ecosystem) -> Kosmos (ecosystem),
    Kosmos (launcher) -> Kepler (launcher).

.DESCRIPTION
    Переносит данные из %APPDATA%\Kepler\ в %APPDATA%\Kosmos\, переименовывает lock-файлы
    и device-id, обновляет HKCU Run entry, переносит установочную директорию launcher'а.

    Скрипт идемпотентен: повторный запуск после успешной миграции ничего не делает.

.PARAMETER WhatIf
    Dry run. Только показывает планируемые действия, ничего не меняет.

.PARAMETER Force
    Игнорировать проверку «target существует и непуст», сливать данные поверх.

.PARAMETER NoStopProcesses
    Не пытаться остановить запущенные процессы перед миграцией.

.PARAMETER SourceOverride
    Внутренний параметр для smoke-теста. Переопределяет $env:APPDATA\Kepler.

.PARAMETER TargetOverride
    Внутренний параметр для smoke-теста. Переопределяет $env:APPDATA\Kosmos.

.PARAMETER InstallerSourceOverride
    Внутренний параметр для smoke-теста. Переопределяет %LOCALAPPDATA%\Kepler\Kosmos.

.PARAMETER InstallerTargetOverride
    Внутренний параметр для smoke-теста. Переопределяет %LOCALAPPDATA%\Kosmos\Kepler.

.PARAMETER SkipRegistry
    Пропустить операции с HKCU Run (для smoke-теста).

.EXAMPLE
    .\migrate-kepler-to-kosmos.ps1 -WhatIf

.EXAMPLE
    .\migrate-kepler-to-kosmos.ps1
#>

[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [switch]$Force,
    [switch]$NoStopProcesses,
    [string]$SourceOverride,
    [string]$TargetOverride,
    [string]$InstallerSourceOverride,
    [string]$InstallerTargetOverride,
    [switch]$SkipRegistry
)

$ErrorActionPreference = 'Stop'

# ---- Конфигурация путей ----

if ($SourceOverride) {
    $source = $SourceOverride
} else {
    $source = Join-Path $env:APPDATA 'Kepler'
}

if ($TargetOverride) {
    $target = $TargetOverride
} else {
    $target = Join-Path $env:APPDATA 'Kosmos'
}

if ($InstallerSourceOverride) {
    $installerSource = $InstallerSourceOverride
} else {
    $installerSource = Join-Path $env:LOCALAPPDATA 'Kepler\Kosmos'
}

if ($InstallerTargetOverride) {
    $installerTarget = $InstallerTargetOverride
} else {
    $installerTarget = Join-Path $env:LOCALAPPDATA 'Kosmos\Kepler'
}

# Карта переименований внутри перемещённой директории.
# Слева — старое имя (kosmos-* — был launcher Kosmos), справа — новое (kepler-* — launcher Kepler).
$renameMap = @{
    'kosmos.lock.json'           = 'kepler.lock.json'
    'kosmos-singleton.lock.db'   = 'kepler-singleton.lock.db'
    'kosmos-device-id.txt'       = 'kepler-device-id.txt'
}

# Имена процессов, которые могут держать БД/файлы (актуальные имена ДО swap'а кода).
$processesToStop = @('kosmos', 'kosmos-backend', 'kosmos-watcher', 'ark-core-rpc', 'usage-tracker')

# Старая запись в реестре до rebrand.
$registryRunPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$oldRunName = 'KeplerKosmos'
$newRunName = 'KosmosKepler'

# Сводка для финального отчёта.
$summary = [ordered]@{
    Moved   = @()
    Renamed = @()
    Skipped = @()
    Failed  = @()
}

function Write-Step {
    param([string]$Message)
    Write-Host "[migrate] $Message" -ForegroundColor Cyan
}

function Write-Note {
    param([string]$Message)
    Write-Host "[migrate] $Message" -ForegroundColor DarkGray
}

function Write-Fail {
    param([string]$Message)
    Write-Host "[migrate] $Message" -ForegroundColor Red
}

# ---- Шаг 1. Проверка состояния ----

Write-Step "Проверка исходной директории: $source"

if (-not (Test-Path -LiteralPath $source)) {
    Write-Host "[migrate] Legacy directory not found at '$source'. Nothing to migrate." -ForegroundColor Green
    exit 0
}

Write-Step "Проверка целевой директории: $target"

$targetExists = Test-Path -LiteralPath $target
$targetNonEmpty = $false
if ($targetExists) {
    $targetNonEmpty = (Get-ChildItem -LiteralPath $target -Force -ErrorAction SilentlyContinue | Measure-Object).Count -gt 0
}

if ($targetNonEmpty -and -not $Force) {
    Write-Fail "Целевая директория '$target' существует и непуста. Запустите с -Force, чтобы выполнить merge."
    Write-Fail "Прервано без изменений."
    exit 2
}

# ---- Шаг 2. Остановка процессов ----

if (-not $NoStopProcesses) {
    Write-Step "Остановка запущенных процессов экосистемы (если есть)"
    foreach ($name in $processesToStop) {
        $running = Get-Process -Name $name -ErrorAction SilentlyContinue
        if ($null -ne $running) {
            if ($PSCmdlet.ShouldProcess($name, 'Stop-Process')) {
                try {
                    Stop-Process -Name $name -Force -ErrorAction Stop
                    Write-Note "Остановлен процесс: $name"
                } catch {
                    Write-Note "Не удалось остановить $name : $($_.Exception.Message)"
                }
            }
        }
    }
    if (-not $WhatIfPreference) {
        Start-Sleep -Seconds 2
    }
} else {
    Write-Note "Пропуск остановки процессов (-NoStopProcesses)."
}

# ---- Шаг 3. Перенос директории ----

Write-Step "Перенос '$source' -> '$target'"

$moveSucceeded = $false
if ($PSCmdlet.ShouldProcess("$source -> $target", 'Move-Item')) {
    try {
        if ($targetNonEmpty -and $Force) {
            # Merge режим: копируем содержимое внутрь target, потом удаляем source.
            Write-Note "Merge-режим: копирование содержимого в существующий target."
            Copy-Item -LiteralPath (Join-Path $source '*') -Destination $target -Recurse -Force -ErrorAction Stop
            Remove-Item -LiteralPath $source -Recurse -Force -ErrorAction Stop
            $moveSucceeded = $true
            $summary.Moved += "$source -> $target (merge)"
        } else {
            if ($targetExists -and -not $targetNonEmpty) {
                # Пустая target директория мешает Move-Item — удаляем её.
                Remove-Item -LiteralPath $target -Force -ErrorAction Stop
            }
            Move-Item -LiteralPath $source -Destination $target -ErrorAction Stop
            $moveSucceeded = $true
            $summary.Moved += "$source -> $target"
        }
    } catch {
        Write-Note "Move-Item не удался: $($_.Exception.Message). Пробуем Copy + Remove."
        try {
            if (-not (Test-Path -LiteralPath $target)) {
                New-Item -ItemType Directory -Path $target -Force | Out-Null
            }
            Copy-Item -LiteralPath (Join-Path $source '*') -Destination $target -Recurse -Force -ErrorAction Stop
            Remove-Item -LiteralPath $source -Recurse -Force -ErrorAction Stop
            $moveSucceeded = $true
            $summary.Moved += "$source -> $target (copy+remove fallback)"
        } catch {
            Write-Fail "Не удалось перенести данные: $($_.Exception.Message)"
            $summary.Failed += "Move $source -> $target : $($_.Exception.Message)"
            exit 3
        }
    }
} else {
    # WhatIf: считаем что произошло.
    $moveSucceeded = $true
    $summary.Moved += "$source -> $target (WhatIf)"
}

# ---- Шаг 4. Переименование файлов внутри ----

if ($moveSucceeded) {
    Write-Step "Переименование lock-файлов и device-id внутри '$target'"
    foreach ($pair in $renameMap.GetEnumerator()) {
        $oldPath = Join-Path $target $pair.Key
        $newPath = Join-Path $target $pair.Value

        if (-not $WhatIfPreference -and -not (Test-Path -LiteralPath $oldPath)) {
            $summary.Skipped += "rename: $($pair.Key) (отсутствует)"
            continue
        }

        if (Test-Path -LiteralPath $newPath) {
            Write-Note "Целевое имя уже существует: $($pair.Value). Пропуск."
            $summary.Skipped += "rename: $($pair.Value) (уже существует)"
            continue
        }

        if ($PSCmdlet.ShouldProcess("$oldPath -> $newPath", 'Rename-Item')) {
            try {
                if (Test-Path -LiteralPath $oldPath) {
                    Rename-Item -LiteralPath $oldPath -NewName $pair.Value -ErrorAction Stop
                    $summary.Renamed += "$($pair.Key) -> $($pair.Value)"
                } else {
                    # WhatIf: файла нет, но мы притворяемся что переименовали.
                    $summary.Renamed += "$($pair.Key) -> $($pair.Value) (WhatIf)"
                }
            } catch {
                Write-Fail "Не удалось переименовать $($pair.Key): $($_.Exception.Message)"
                $summary.Failed += "rename $($pair.Key) : $($_.Exception.Message)"
            }
        }
    }
}

# ---- Шаг 5. HKCU Run ----

if (-not $SkipRegistry) {
    Write-Step "Обновление HKCU Run entry"
    try {
        $existingOld = Get-ItemProperty -Path $registryRunPath -Name $oldRunName -ErrorAction SilentlyContinue
        if ($null -ne $existingOld) {
            if ($PSCmdlet.ShouldProcess("$registryRunPath\$oldRunName", 'Remove-ItemProperty')) {
                Remove-ItemProperty -Path $registryRunPath -Name $oldRunName -ErrorAction Stop
                $summary.Renamed += "HKCU Run: удалён $oldRunName"
            }
        } else {
            $summary.Skipped += "HKCU Run: $oldRunName отсутствует"
        }
    } catch {
        Write-Fail "Не удалось удалить старую HKCU Run запись: $($_.Exception.Message)"
        $summary.Failed += "HKCU $oldRunName : $($_.Exception.Message)"
    }

    $newLauncherExe = Join-Path $installerTarget 'kepler.exe'
    if (Test-Path -LiteralPath $newLauncherExe) {
        try {
            if ($PSCmdlet.ShouldProcess("$registryRunPath\$newRunName", 'Set-ItemProperty')) {
                Set-ItemProperty -Path $registryRunPath -Name $newRunName -Value "`"$newLauncherExe`"" -ErrorAction Stop
                $summary.Renamed += "HKCU Run: добавлен $newRunName -> $newLauncherExe"
            }
        } catch {
            Write-Fail "Не удалось записать новую HKCU Run запись: $($_.Exception.Message)"
            $summary.Failed += "HKCU $newRunName : $($_.Exception.Message)"
        }
    } else {
        Write-Note "Новый launcher не найден в '$newLauncherExe'. Пропуск создания HKCU Run."
        Write-Note "После установки нового Kepler launcher autorun можно настроить вручную."
        $summary.Skipped += "HKCU Run: $newRunName (launcher не установлен)"
    }
} else {
    Write-Note "Пропуск операций с реестром (-SkipRegistry)."
}

# ---- Шаг 6. Установочная директория ----

Write-Step "Проверка установочной директории launcher: $installerSource"
if (Test-Path -LiteralPath $installerSource) {
    if (Test-Path -LiteralPath $installerTarget) {
        Write-Note "Целевая установочная директория уже существует: $installerTarget. Пропуск."
        $summary.Skipped += "installer: $installerTarget (уже существует)"
    } else {
        if ($PSCmdlet.ShouldProcess("$installerSource -> $installerTarget", 'Move-Item')) {
            try {
                $installerParent = Split-Path -Parent $installerTarget
                if (-not (Test-Path -LiteralPath $installerParent)) {
                    New-Item -ItemType Directory -Path $installerParent -Force | Out-Null
                }
                Move-Item -LiteralPath $installerSource -Destination $installerTarget -ErrorAction Stop
                $summary.Moved += "$installerSource -> $installerTarget"
            } catch {
                Write-Fail "Не удалось перенести installer dir: $($_.Exception.Message)"
                $summary.Failed += "installer move : $($_.Exception.Message)"
            }
        }
    }
} else {
    Write-Note "Установочная директория не найдена: $installerSource. Пропуск."
    $summary.Skipped += "installer: $installerSource (отсутствует)"
}

# ---- Шаг 7. Сводка ----

Write-Host ''
Write-Host "==== Сводка миграции ====" -ForegroundColor Yellow

Write-Host "Перенесено:" -ForegroundColor Green
if ($summary.Moved.Count -eq 0) {
    Write-Host "  (ничего)"
} else {
    $summary.Moved | ForEach-Object { Write-Host "  $_" }
}

Write-Host "Переименовано:" -ForegroundColor Green
if ($summary.Renamed.Count -eq 0) {
    Write-Host "  (ничего)"
} else {
    $summary.Renamed | ForEach-Object { Write-Host "  $_" }
}

Write-Host "Пропущено:" -ForegroundColor DarkGray
if ($summary.Skipped.Count -eq 0) {
    Write-Host "  (ничего)"
} else {
    $summary.Skipped | ForEach-Object { Write-Host "  $_" }
}

Write-Host "Ошибки:" -ForegroundColor Red
if ($summary.Failed.Count -eq 0) {
    Write-Host "  (нет)"
} else {
    $summary.Failed | ForEach-Object { Write-Host "  $_" }
}

if ($summary.Failed.Count -gt 0) {
    exit 4
}

exit 0
