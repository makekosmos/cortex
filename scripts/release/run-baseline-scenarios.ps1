<#
.SYNOPSIS
    Orchestrator: запускает full baseline RAM measurement по 4 сценариям.

.DESCRIPTION
    1. launcher-only       — голый Kepler, без extensions
    2. all-extensions-idle — Kepler + 4 ext + Dashboard, 5min idle (KEPLER_BENCHMARK_OPEN_ALL=1)
    3. exp23-acrylic       — Kepler launcher с backgroundMaterial=acrylic (default)
    4. exp23-mica          — Kepler launcher с backgroundMaterial=mica

    Каждый сценарий: оператор стартует Kepler с соответствующими env vars,
    скрипт измеряет, оператор останавливает. Скрипт сам не запускает Kepler —
    бесшумно прокликивать UI не получится, но он орchestrir'ует измерения и
    дампит сравнительный отчёт.

    После всех 4 запусков → markdown report в .agent/tasks/<DATE>-ram-baseline/.

.PARAMETER Scenarios
    Список scenarios для запуска. Default = все 4.

.PARAMETER Warmup
    Сколько секунд ждать перед snapshot (default 30 для idle стабилизации).

.PARAMETER Samples
    Сколько snapshot'ов снять per scenario (default 3 для усреднения).

.PARAMETER SampleInterval
    Интервал между samples (default 10s).

.EXAMPLE
    # All scenarios sequentially (нужно вручную перезапускать Kepler между ними)
    pwsh scripts/release/run-baseline-scenarios.ps1

.EXAMPLE
    # Только один сценарий
    pwsh scripts/release/run-baseline-scenarios.ps1 -Scenarios all-extensions-idle
#>

[CmdletBinding()]
param(
    [string[]]$Scenarios = @('launcher-only','all-extensions-idle'),
    [int]$Warmup = 30,
    [int]$Samples = 3,
    [int]$SampleInterval = 10
)

$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "=== Kepler baseline scenarios ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Этот скрипт прогонит измерения для следующих сценариев:" -ForegroundColor Yellow
$Scenarios | ForEach-Object { Write-Host "  - $_" }
Write-Host ""

foreach ($scenario in $Scenarios) {
    $envHint = switch ($scenario) {
        'launcher-only'        { '(нет env vars)' }
        'all-extensions-idle'  { '$env:KEPLER_BENCHMARK_OPEN_ALL=''1''' }
        default                { '($scenario нестандартный — сам задай env)' }
    }

    Write-Host ""
    Write-Host ("--- Сценарий: {0} ---" -f $scenario) -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Подготовка:" -ForegroundColor Yellow
    Write-Host ("  1. Закрой все процессы Kepler (если запущены).")
    Write-Host ("  2. В отдельном окне: ")
    Write-Host ("     {0}" -f $envHint)
    Write-Host ("     bun run --cwd desktop dev")
    if ($scenario -eq 'all-extensions-idle') {
        Write-Host ("  3. Дождись пока launcher автоматически откроет все 4 extensions + dashboard (~5s).")
    } elseif ($scenario -ne 'launcher-only') {
        Write-Host ("  3. Открой launcher (Ctrl+Shift+K), оставь видимым.")
    }
    Write-Host ""
    Write-Host "Нажми Enter когда Kepler готов и стабилизировался..." -ForegroundColor Green
    [void](Read-Host)

    Write-Host ("[{0}] Замеряем (warmup {1}s, {2} samples × {3}s interval)..." -f $scenario, $Warmup, $Samples, $SampleInterval) -ForegroundColor Cyan

    & pwsh -NoProfile -ExecutionPolicy Bypass `
        (Join-Path $scriptDir 'measure-kepler-ram.ps1') `
        -Mode kepler `
        -Scenario $scenario `
        -Warmup $Warmup `
        -Samples $Samples `
        -SampleInterval $SampleInterval
}

Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Green
Write-Host "Все отчёты в .tmp/ram-kepler-<scenario>-<ts>.json" -ForegroundColor Yellow
