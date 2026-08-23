<#
.SYNOPSIS
    Smoke-тест миграционного скрипта migrate-kepler-to-kosmos.ps1.

.DESCRIPTION
    Создаёт временную песочницу, имитирует структуру %APPDATA%\Kepler\
    с поддельными lock-файлами и ark.db, запускает миграцию с override-путями,
    проверяет результат. Реестр и реальные пользовательские данные не трогаются.

.EXAMPLE
    .\migrate-kepler-to-kosmos-smoke.ps1
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$migrate = Join-Path $scriptDir 'migrate-kepler-to-kosmos.ps1'

if (-not (Test-Path -LiteralPath $migrate)) {
    Write-Host "[smoke] Не найден скрипт миграции: $migrate" -ForegroundColor Red
    exit 1
}

$sandboxRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("kepler-migrate-smoke-" + [Guid]::NewGuid().ToString('N'))
$source = Join-Path $sandboxRoot 'AppData\Kepler'
$target = Join-Path $sandboxRoot 'AppData\Kosmos'
$installerSource = Join-Path $sandboxRoot 'LocalAppData\Kepler\Kosmos'
$installerTarget = Join-Path $sandboxRoot 'LocalAppData\Kosmos\Kepler'

$failures = New-Object System.Collections.Generic.List[string]

function Assert-True {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) {
        $failures.Add($Message)
        Write-Host "[smoke] FAIL: $Message" -ForegroundColor Red
    } else {
        Write-Host "[smoke]  OK : $Message" -ForegroundColor Green
    }
}

try {
    Write-Host "[smoke] Sandbox: $sandboxRoot" -ForegroundColor Cyan

    # ---- Подготовка песочницы ----

    New-Item -ItemType Directory -Path $source -Force | Out-Null
    New-Item -ItemType Directory -Path $installerSource -Force | Out-Null

    # Имитация файлов экосистемы.
    Set-Content -LiteralPath (Join-Path $source 'kosmos.lock.json') -Value '{"pid":0}' -Encoding utf8
    Set-Content -LiteralPath (Join-Path $source 'kosmos-device-id.txt') -Value 'test-device-id-12345' -Encoding utf8
    # ark.db делаем пустым бинарником, чтобы убедиться, что Move сохраняет содержимое.
    [System.IO.File]::WriteAllBytes((Join-Path $source 'ark.db'), [byte[]](0x53, 0x51, 0x4C, 0x69, 0x74, 0x65))
    # singleton lock — просто файл.
    Set-Content -LiteralPath (Join-Path $source 'kosmos-singleton.lock.db') -Value 'lock' -Encoding utf8

    # Вложенная директория с пользовательскими данными — должна переехать целиком.
    $subDir = Join-Path $source 'eden\vault'
    New-Item -ItemType Directory -Path $subDir -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $subDir 'note.md') -Value '# my note' -Encoding utf8

    # Installer dir.
    Set-Content -LiteralPath (Join-Path $installerSource 'kosmos.exe') -Value 'binary-placeholder' -Encoding utf8

    # ---- Запуск миграции ----

    Write-Host "[smoke] Запуск migrate-kepler-to-kosmos.ps1 ..." -ForegroundColor Cyan

    & $migrate `
        -SourceOverride $source `
        -TargetOverride $target `
        -InstallerSourceOverride $installerSource `
        -InstallerTargetOverride $installerTarget `
        -NoStopProcesses `
        -SkipRegistry

    $exit = $LASTEXITCODE
    Assert-True ($exit -eq 0) "migrate exit code = 0 (фактически $exit)"

    # ---- Проверки ----

    Assert-True (-not (Test-Path -LiteralPath $source)) "Source удалён: $source"
    Assert-True (Test-Path -LiteralPath $target) "Target существует: $target"

    Assert-True (Test-Path -LiteralPath (Join-Path $target 'kepler.lock.json')) "kepler.lock.json создан"
    Assert-True (Test-Path -LiteralPath (Join-Path $target 'kepler-device-id.txt')) "kepler-device-id.txt создан"
    Assert-True (Test-Path -LiteralPath (Join-Path $target 'kepler-singleton.lock.db')) "kepler-singleton.lock.db создан"

    Assert-True (-not (Test-Path -LiteralPath (Join-Path $target 'kosmos.lock.json'))) "kosmos.lock.json удалён"
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $target 'kosmos-device-id.txt'))) "kosmos-device-id.txt удалён"
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $target 'kosmos-singleton.lock.db'))) "kosmos-singleton.lock.db удалён"

    Assert-True (Test-Path -LiteralPath (Join-Path $target 'ark.db')) "ark.db перенесён"
    Assert-True (Test-Path -LiteralPath (Join-Path $target 'eden\vault\note.md')) "Вложенная структура eden/vault/note.md сохранена"

    $deviceId = Get-Content -LiteralPath (Join-Path $target 'kepler-device-id.txt') -Raw
    Assert-True ($deviceId.Trim() -eq 'test-device-id-12345') "Содержимое device-id сохранилось"

    Assert-True (Test-Path -LiteralPath $installerTarget) "Installer target существует"
    Assert-True (Test-Path -LiteralPath (Join-Path $installerTarget 'kosmos.exe')) "Installer файл перенесён"
    Assert-True (-not (Test-Path -LiteralPath $installerSource)) "Installer source удалён"

    # ---- Идемпотентность ----

    Write-Host "[smoke] Проверка идемпотентности (второй запуск)..." -ForegroundColor Cyan
    & $migrate `
        -SourceOverride $source `
        -TargetOverride $target `
        -InstallerSourceOverride $installerSource `
        -InstallerTargetOverride $installerTarget `
        -NoStopProcesses `
        -SkipRegistry
    Assert-True ($LASTEXITCODE -eq 0) "Повторный запуск exit code = 0 (фактически $LASTEXITCODE)"

} finally {
    # ---- Cleanup ----
    if (Test-Path -LiteralPath $sandboxRoot) {
        try {
            Remove-Item -LiteralPath $sandboxRoot -Recurse -Force -ErrorAction Stop
            Write-Host "[smoke] Sandbox очищен." -ForegroundColor DarkGray
        } catch {
            Write-Host "[smoke] Не удалось очистить sandbox: $($_.Exception.Message)" -ForegroundColor Yellow
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Host ''
    Write-Host "[smoke] Провалено проверок: $($failures.Count)" -ForegroundColor Red
    $failures | ForEach-Object { Write-Host "  - $_" -ForegroundColor Red }
    exit 1
}

Write-Host ''
Write-Host "[smoke] Все проверки пройдены." -ForegroundColor Green
exit 0
