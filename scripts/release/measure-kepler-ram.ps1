<#
.SYNOPSIS
    Измерение RAM-потребления Kepler ecosystem vs. набора standalone-апок (baseline).

.DESCRIPTION
    Снимает snapshot WorkingSet64 / PrivateMemorySize64 для интересующих процессов
    после фиксированного warmup-периода и сохраняет результат в JSON.

    Два режима:
      - baseline: измеряет standalone-апки (Eden, Delphi, Arrancador)
        плюс их child-процессы (ark-core-rpc) — фотография «до Kepler ecosystem».
      - kepler: измеряет Kepler ecosystem (kepler-shell + Electron child-процессы
        + kepler-backend с in-process ARK) — фотография «после Phase 1».
        ark-core-rpc больше не child-процесс Engine; если leftover старой
        установки ещё жив, он попадёт в отчёт отдельной строкой.

    Дополнительно режим -Compare читает последние два JSON-отчёта из .tmp и печатает diff.

.PARAMETER Mode
    baseline | kepler. По умолчанию kepler.

.PARAMETER Duration
    Зарезервировано (один snapshot после warmup). По умолчанию 60.

.PARAMETER Warmup
    Сколько секунд ждать перед snapshot. По умолчанию 15.

.PARAMETER OutFile
    Путь к JSON-отчёту. По умолчанию .tmp/ram-measurement-<mode>-<timestamp>.json.

.PARAMETER Compare
    Прочитать последние baseline + kepler отчёты из .tmp и напечатать diff.

.EXAMPLE
    pwsh scripts/release/measure-kepler-ram.ps1 -Mode baseline

.EXAMPLE
    pwsh scripts/release/measure-kepler-ram.ps1 -Mode kepler -Warmup 20

.EXAMPLE
    pwsh scripts/release/measure-kepler-ram.ps1 -Compare
#>

[CmdletBinding()]
param(
    [ValidateSet('baseline', 'kepler')]
    [string]$Mode = 'kepler',

    [int]$Duration = 60,

    [int]$Warmup = 15,

    [string]$OutFile,

    [switch]$Compare,

    # Метка сценария — попадает в JSON и в filename. Примеры:
    #   "launcher-only"        — голый Kepler без extensions
    #   "all-extensions-idle"  — launcher + 4 ext + dashboard, 5min idle
    #   "exp23-acrylic"        — A/B Mica vs Acrylic, acrylic вариант
    #   "exp23-mica"           — то же, mica
    [string]$Scenario = 'default',

    # Сколько snapshot'ов снять с интервалом $SampleInterval секунд.
    # >1 даёт mean/median/min/max — устойчиво к моментальным GC скачкам.
    [int]$Samples = 1,

    # Интервал между samples в секундах.
    [int]$SampleInterval = 5
)

$ErrorActionPreference = 'Stop'

$cortexRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

# --- Конфигурация ролей ---------------------------------------------------

# Baseline: standalone-апки (имена .exe без расширения, как видит Get-Process).
# Имена case-insensitive в Windows, поэтому достаточно по одному варианту.
$BaselineRoleMap = @{
    'eden'           = 'eden'
    'delphi'         = 'delphi'
    'arrancador'     = 'arrancador'
    'ark-core-rpc'   = 'ark-core-rpc'
}

# Kepler ecosystem: главный shell, backend (ARK теперь in-process).
# 'kepler-shell' — dev mode (vite + electron-vite spawn).
# 'Kepler'       — production build (electron-builder NSIS productName).
# 'ark-core-rpc' — sidecar старых (pre-in-process) установок: не обязателен,
#                  но если процесс жив, его тоже замеряем для сравнения.
$KeplerMainRoleMap = @{
    'kepler-shell'   = 'electron-main'
    'Kepler'         = 'electron-main'
    'kepler-backend' = 'kepler-backend'
    'ark-core-rpc'   = 'ark-core-rpc'
}

# Helper: bytes -> MB (1 decimal).
function To-MB {
    param([long]$Bytes)
    return [Math]::Round($Bytes / 1MB, 1)
}

function Get-ProcessTree {
    <#
    .SYNOPSIS
        Возвращает все процессы — root + рекурсивные дети.
    #>
    param(
        [Parameter(Mandatory)][int[]]$RootPids
    )

    $allWmi = Get-CimInstance Win32_Process -ErrorAction Stop
    $byParent = @{}
    foreach ($p in $allWmi) {
        $key = [int]$p.ParentProcessId
        if (-not $byParent.ContainsKey($key)) {
            $byParent[$key] = New-Object System.Collections.ArrayList
        }
        [void]$byParent[$key].Add($p)
    }

    $visited = @{}
    $queue = New-Object System.Collections.Queue
    foreach ($procId in $RootPids) {
        $queue.Enqueue([int]$procId)
    }

    $result = New-Object System.Collections.ArrayList
    while ($queue.Count -gt 0) {
        $current = [int]$queue.Dequeue()
        if ($visited.ContainsKey($current)) { continue }
        $visited[$current] = $true

        $wmi = $allWmi | Where-Object { $_.ProcessId -eq $current } | Select-Object -First 1
        if ($null -ne $wmi) {
            [void]$result.Add($wmi)
        }

        if ($byParent.ContainsKey($current)) {
            foreach ($child in $byParent[$current]) {
                $queue.Enqueue([int]$child.ProcessId)
            }
        }
    }

    return $result
}

function Classify-KeplerChild {
    <#
    .SYNOPSIS
        Определяет роль дочернего процесса Electron по CommandLine.
    #>
    param([string]$CommandLine)

    if ([string]::IsNullOrWhiteSpace($CommandLine)) { return 'electron-utility' }

    $cl = $CommandLine.ToLowerInvariant()
    if ($cl -match '--type=renderer') { return 'electron-renderer' }
    if ($cl -match '--type=gpu-process') { return 'electron-gpu' }
    if ($cl -match '--type=utility') { return 'electron-utility' }
    if ($cl -match '--type=zygote') { return 'electron-utility' }
    if ($cl -match '--type=crashpad-handler') { return 'electron-utility' }
    if ($cl -match '--type=broker') { return 'electron-utility' }
    return 'electron-utility'
}

function Get-ProcessSnapshot {
    <#
    .SYNOPSIS
        Снимает WorkingSet64 + PrivateMemorySize64 для PID'а через Get-Process.
    #>
    param([int]$ProcessId)

    try {
        $proc = Get-Process -Id $ProcessId -ErrorAction Stop
        return [PSCustomObject]@{
            pid                = $ProcessId
            name               = $proc.ProcessName
            working_set_bytes  = [long]$proc.WorkingSet64
            private_bytes      = [long]$proc.PrivateMemorySize64
        }
    } catch {
        return $null
    }
}

function Collect-Baseline {
    Write-Host "[baseline] Поиск standalone-процессов..." -ForegroundColor Cyan

    $rootPids = New-Object System.Collections.ArrayList
    $pidToRole = @{}

    foreach ($procName in $BaselineRoleMap.Keys) {
        $found = Get-Process -Name $procName -ErrorAction SilentlyContinue
        foreach ($p in $found) {
            if (-not $pidToRole.ContainsKey($p.Id)) {
                $pidToRole[$p.Id] = $BaselineRoleMap[$procName]
                [void]$rootPids.Add($p.Id)
            }
        }
    }

    if ($rootPids.Count -eq 0) {
        Write-Host ""
        Write-Host "Не найдено ни одного standalone-процесса." -ForegroundColor Yellow
        Write-Host "Запусти вручную нужные апки (Eden / Delphi / Arrancador)," -ForegroundColor Yellow
        Write-Host "дождись полной загрузки и повтори:" -ForegroundColor Yellow
        Write-Host "    pwsh scripts/release/measure-kepler-ram.ps1 -Mode baseline" -ForegroundColor Yellow
        exit 2
    }

    Write-Host ("[baseline] Найдено корневых процессов: {0}" -f $rootPids.Count)
    Write-Host ("[baseline] Warmup {0}s..." -f $Warmup)
    Start-Sleep -Seconds $Warmup

    # Собираем все процессы + детей (на случай если Electron-апки тоже spawn-ят renderer-ов).
    $tree = Get-ProcessTree -RootPids $rootPids

    $samples = New-Object System.Collections.ArrayList
    foreach ($wmi in $tree) {
        $snap = Get-ProcessSnapshot -ProcessId ([int]$wmi.ProcessId)
        if ($null -eq $snap) { continue }

        # Роль: если root — берём из map; если ребёнок — это electron-renderer/gpu/utility/...
        $role = $null
        if ($pidToRole.ContainsKey([int]$wmi.ProcessId)) {
            $role = $pidToRole[[int]$wmi.ProcessId]
        } else {
            # Ребёнок Electron-апки.
            $role = Classify-KeplerChild -CommandLine $wmi.CommandLine
        }

        $obj = [PSCustomObject]@{
            pid                = $snap.pid
            name               = $snap.name
            role               = $role
            working_set_bytes  = $snap.working_set_bytes
            private_bytes      = $snap.private_bytes
        }
        [void]$samples.Add($obj)
    }

    return ,$samples
}

function Collect-Kepler {
    Write-Host "[kepler] Поиск процессов Kepler ecosystem..." -ForegroundColor Cyan

    $rootPids = New-Object System.Collections.ArrayList
    $pidToRole = @{}

    foreach ($procName in $KeplerMainRoleMap.Keys) {
        $found = Get-Process -Name $procName -ErrorAction SilentlyContinue
        foreach ($p in $found) {
            if (-not $pidToRole.ContainsKey($p.Id)) {
                $pidToRole[$p.Id] = $KeplerMainRoleMap[$procName]
                [void]$rootPids.Add($p.Id)
            }
        }
    }

    $hasShell    = $pidToRole.Values -contains 'electron-main'
    $hasBackend  = $pidToRole.Values -contains 'kepler-backend'

    if (-not $hasShell -or -not $hasBackend) {
        Write-Host ""
        Write-Host "Не все ожидаемые процессы Kepler ecosystem запущены." -ForegroundColor Yellow
        Write-Host ("    kepler-shell.exe   : {0}" -f $(if ($hasShell)   { 'OK' } else { 'НЕ НАЙДЕН' }))
        Write-Host ("    kepler-backend.exe : {0}" -f $(if ($hasBackend) { 'OK' } else { 'НЕ НАЙДЕН' }))
        Write-Host ""
        Write-Host "Запусти Kepler ecosystem (kepler-shell), дождись полной загрузки и повтори:" -ForegroundColor Yellow
        Write-Host "    pwsh scripts/release/measure-kepler-ram.ps1 -Mode kepler" -ForegroundColor Yellow
        exit 2
    }

    Write-Host ("[kepler] Найдено корневых процессов: {0}" -f $rootPids.Count)
    Write-Host ("[kepler] Warmup {0}s..." -f $Warmup)
    Start-Sleep -Seconds $Warmup

    $tree = Get-ProcessTree -RootPids $rootPids

    $samples = New-Object System.Collections.ArrayList
    foreach ($wmi in $tree) {
        $snap = Get-ProcessSnapshot -ProcessId ([int]$wmi.ProcessId)
        if ($null -eq $snap) { continue }

        $role = $null
        if ($pidToRole.ContainsKey([int]$wmi.ProcessId)) {
            $role = $pidToRole[[int]$wmi.ProcessId]
        } else {
            $role = Classify-KeplerChild -CommandLine $wmi.CommandLine
        }

        $obj = [PSCustomObject]@{
            pid                = $snap.pid
            name               = $snap.name
            role               = $role
            working_set_bytes  = $snap.working_set_bytes
            private_bytes      = $snap.private_bytes
        }
        [void]$samples.Add($obj)
    }

    return ,$samples
}

function Print-Summary {
    param(
        [Parameter(Mandatory)][System.Collections.IEnumerable]$Samples,
        [Parameter(Mandatory)][string]$Mode
    )

    Write-Host ""
    Write-Host ("=== Сводка ({0}) ===" -f $Mode) -ForegroundColor Green

    $byRole = @{}
    foreach ($s in $Samples) {
        if (-not $byRole.ContainsKey($s.role)) {
            $byRole[$s.role] = [PSCustomObject]@{
                count   = 0
                rss     = [long]0
                private = [long]0
            }
        }
        $byRole[$s.role].count   += 1
        $byRole[$s.role].rss     += $s.working_set_bytes
        $byRole[$s.role].private += $s.private_bytes
    }

    $rows = foreach ($role in ($byRole.Keys | Sort-Object)) {
        $r = $byRole[$role]
        [PSCustomObject]@{
            Role       = $role
            Count      = $r.count
            'RSS, MB'  = (To-MB -Bytes $r.rss)
            'Priv, MB' = (To-MB -Bytes $r.private)
        }
    }
    $rows | Format-Table -AutoSize | Out-Host

    $totalRss     = ($Samples | Measure-Object -Property working_set_bytes -Sum).Sum
    $totalPrivate = ($Samples | Measure-Object -Property private_bytes -Sum).Sum
    Write-Host ("ИТОГО RSS:     {0} MB" -f (To-MB -Bytes $totalRss)) -ForegroundColor Green
    Write-Host ("ИТОГО Private: {0} MB" -f (To-MB -Bytes $totalPrivate)) -ForegroundColor Green

    return [PSCustomObject]@{
        working_set_mb = (To-MB -Bytes $totalRss)
        private_mb     = (To-MB -Bytes $totalPrivate)
    }
}

function Save-Report {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Mode,
        [Parameter(Mandatory)][int]$WarmupSeconds,
        [Parameter(Mandatory)][System.Collections.IEnumerable]$Samples,
        [Parameter(Mandatory)][PSCustomObject]$Totals
    )

    $parent = Split-Path -Parent $Path
    if ($parent -and -not (Test-Path $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }

    $report = [PSCustomObject]@{
        mode      = $Mode
        scenario  = $Scenario
        timestamp = (Get-Date).ToString('o')
        warmup_s  = $WarmupSeconds
        bg_material = ($env:KEPLER_BG_MATERIAL ?? 'acrylic')
        processes = @($Samples)
        totals    = $Totals
    }

    $json = $report | ConvertTo-Json -Depth 6
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $json, $utf8NoBom)

    Write-Host ""
    Write-Host ("Отчёт сохранён: {0}" -f $Path) -ForegroundColor Green
}

function Invoke-Compare {
    $tmpDir = Join-Path $cortexRoot '.tmp'
    if (-not (Test-Path $tmpDir)) {
        Write-Host "Папка .tmp не существует — сначала сделай два запуска (baseline + kepler)." -ForegroundColor Yellow
        exit 2
    }

    $baselineLatest = Get-ChildItem -Path $tmpDir -Filter 'ram-measurement-baseline-*.json' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $keplerLatest = Get-ChildItem -Path $tmpDir -Filter 'ram-measurement-kepler-*.json' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1

    if ($null -eq $baselineLatest -or $null -eq $keplerLatest) {
        Write-Host "Не найдено по одному отчёту каждого типа в .tmp." -ForegroundColor Yellow
        Write-Host ("    baseline: {0}" -f $(if ($baselineLatest) { $baselineLatest.Name } else { 'нет' }))
        Write-Host ("    kepler:   {0}" -f $(if ($keplerLatest)   { $keplerLatest.Name }   else { 'нет' }))
        exit 2
    }

    $baseline = Get-Content $baselineLatest.FullName -Raw | ConvertFrom-Json
    $kepler   = Get-Content $keplerLatest.FullName   -Raw | ConvertFrom-Json

    $bRss = [double]$baseline.totals.working_set_mb
    $kRss = [double]$kepler.totals.working_set_mb
    $diff = $bRss - $kRss
    $pct  = if ($bRss -gt 0) { [Math]::Round(($diff / $bRss) * 100, 1) } else { 0 }

    Write-Host ""
    Write-Host "=== Сравнение ===" -ForegroundColor Green
    Write-Host ("Baseline ({0}): RSS = {1} MB, Private = {2} MB" -f $baselineLatest.Name, $baseline.totals.working_set_mb, $baseline.totals.private_mb)
    Write-Host ("Kepler   ({0}): RSS = {1} MB, Private = {2} MB" -f $keplerLatest.Name,   $kepler.totals.working_set_mb,   $kepler.totals.private_mb)
    Write-Host ""
    if ($diff -gt 0) {
        Write-Host ("Экономия: {0} MB ({1}%)" -f $diff, $pct) -ForegroundColor Green
    } elseif ($diff -lt 0) {
        Write-Host ("Регрессия: Kepler потребляет на {0} MB больше ({1}%)" -f ([Math]::Abs($diff)), ([Math]::Abs($pct))) -ForegroundColor Red
    } else {
        Write-Host "Различий нет." -ForegroundColor Yellow
    }
}

# --- Main -----------------------------------------------------------------

if ($Compare) {
    Invoke-Compare
    return
}

if ([string]::IsNullOrWhiteSpace($OutFile)) {
    $ts = (Get-Date).ToString('yyyyMMdd-HHmmss')
    $OutFile = Join-Path $cortexRoot (".tmp/ram-{0}-{1}-{2}.json" -f $Mode, $Scenario, $ts)
}

# Multi-sample: усредняем totals по $Samples snapshot'ам с интервалом
# $SampleInterval. Это сглаживает моментальные GC скачки.
$allTotals = New-Object System.Collections.ArrayList
$lastSamples = $null
for ($i = 0; $i -lt $Samples; $i++) {
    if ($i -gt 0) {
        Write-Host ("[sample {0}/{1}] sleep {2}s..." -f ($i+1), $Samples, $SampleInterval) -ForegroundColor DarkGray
        Start-Sleep -Seconds $SampleInterval
    }
    $snap = switch ($Mode) {
        'baseline' { Collect-Baseline }
        'kepler'   { Collect-Kepler }
    }
    if ($snap -and @($snap).Count -gt 0) {
        $lastSamples = $snap
        $sumWs = ($snap | Measure-Object -Property working_set_bytes -Sum).Sum
        $sumPb = ($snap | Measure-Object -Property private_bytes -Sum).Sum
        [void]$allTotals.Add([PSCustomObject]@{
            working_set_bytes = [long]$sumWs
            private_bytes     = [long]$sumPb
        })
    }
}

if ($null -eq $lastSamples -or @($lastSamples).Count -eq 0) {
    Write-Host "Не собрано ни одного сэмпла." -ForegroundColor Red
    exit 1
}

$meanWs = [Math]::Round((($allTotals | Measure-Object -Property working_set_bytes -Average).Average) / 1MB, 1)
$meanPb = [Math]::Round((($allTotals | Measure-Object -Property private_bytes -Average).Average) / 1MB, 1)

Write-Host ""
Write-Host ("[multi-sample] {0} snapshots, mean RSS = {1} MB, mean Private = {2} MB" -f $allTotals.Count, $meanWs, $meanPb) -ForegroundColor Cyan

$totals = Print-Summary -Samples $lastSamples -Mode $Mode
$totals | Add-Member -NotePropertyName 'mean_working_set_mb' -NotePropertyValue $meanWs -Force
$totals | Add-Member -NotePropertyName 'mean_private_mb'     -NotePropertyValue $meanPb -Force
$totals | Add-Member -NotePropertyName 'samples_taken'       -NotePropertyValue $allTotals.Count -Force
$totals | Add-Member -NotePropertyName 'scenario'            -NotePropertyValue $Scenario -Force

Save-Report -Path $OutFile -Mode $Mode -WarmupSeconds $Warmup -Samples $lastSamples -Totals $totals
