<#
.SYNOPSIS
    Проверка полноты swap Kepler <-> Kosmos в кодовой базе.

.DESCRIPTION
    Ищет оставшиеся ссылки на «старые» (pre-swap) tokens, которые после
    переименования не должны существовать в репозитории. Исключает
    target/, node_modules/, dist/, build/, .git/, сами миграционные скрипты
    и ADR миграции.

    Выходной код:
      0 — оставшихся ссылок не найдено;
      1 — найдены нарушения, печатает список.

.PARAMETER Root
    Корень репозитория. По умолчанию — родительская папка скрипта.

.PARAMETER ShowContext
    Печатать совпавшую строку рядом с file:line.

.EXAMPLE
    pwsh -File scripts\check-swap-completeness.ps1

.EXAMPLE
    pwsh -File scripts\check-swap-completeness.ps1 -ShowContext
#>

[CmdletBinding()]
param(
    [string]$Root,
    [switch]$ShowContext
)

$ErrorActionPreference = 'Stop'

if (-not $Root) {
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $Root = Split-Path -Parent $scriptDir
}

$Root = (Resolve-Path -LiteralPath $Root).Path
Write-Host "[check-swap] Корень: $Root" -ForegroundColor Cyan

# Запрещённые токены после swap. Каждый — pattern + описание.
# Pattern — regex (Select-String SimpleMatch=false). Точные строковые совпадения
# даны в литеральном виде с экранированием спецсимволов где нужно.
$forbiddenPatterns = @(
    @{ Pattern = '@kepler/ark';                Reason = 'Должен быть @kosmos/ark' }
    @{ Pattern = '@kepler/visuals';            Reason = 'Должен быть @kosmos/visuals' }
    @{ Pattern = '@kepler/';                   Reason = 'NPM scope @kepler/ должен быть @kosmos/' }
    @{ Pattern = 'KEPLER_DB_PATH';             Reason = 'Env var должен быть KOSMOS_DB_PATH' }
    @{ Pattern = 'KEPLER_SPACE_ID';            Reason = 'Env var должен быть KOSMOS_SPACE_ID' }
    @{ Pattern = 'KEPLER_HOME';                Reason = 'Env var должен быть KOSMOS_HOME' }
    @{ Pattern = 'kosmos\.lock\.json';         Reason = 'Lock-файл launcher теперь kepler.lock.json' }
    @{ Pattern = 'kosmos-singleton\.lock\.db'; Reason = 'Singleton lock теперь kepler-singleton.lock.db' }
    @{ Pattern = 'kosmos-device-id\.txt';      Reason = 'Device id теперь kepler-device-id.txt' }
    @{ Pattern = 'services/kosmos-backend';    Reason = 'Должен быть services/kepler-backend' }
    @{ Pattern = 'services\\kosmos-backend';   Reason = 'Должен быть services\\kepler-backend (Windows path)' }
    @{ Pattern = 'kosmos-backend';             Reason = 'Backend service переименован в kepler-backend' }
    @{ Pattern = 'KeplerKosmos';               Reason = 'HKCU Run key теперь KosmosKepler' }
    @{ Pattern = 'APPDATA.*\\Kepler';          Reason = 'Ecosystem data dir теперь %APPDATA%\Kosmos' }
    @{ Pattern = 'LOCALAPPDATA.*\\Kepler\\Kosmos'; Reason = 'Installer dir теперь %LOCALAPPDATA%\Kosmos\Kepler' }
)

# Исключения путей. Glob/литеральные сегменты.
$excludeDirs = @(
    'node_modules',
    '.git',
    'target',
    'dist',
    'build',
    'coverage',
    '.tmp',
    '.e2e',
    '.next',
    '.vitepress\dist',
    '.cache'
)

# Файлы-исключения: миграционные артефакты, которые ДОЛЖНЫ упоминать старые токены.
$excludeFiles = @(
    'scripts\migrate-kepler-to-kosmos.ps1',
    'scripts\migrate-kepler-to-kosmos-smoke.ps1',
    'scripts\check-swap-completeness.ps1',
    'docs\MIGRATION-2026-05-14-brand-swap.md'
)

# Расширения, которые имеет смысл сканировать.
$textExtensions = @(
    '.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs',
    '.vue', '.svelte', '.html', '.css', '.scss',
    '.rs', '.toml', '.json', '.jsonc',
    '.md', '.mdx', '.txt',
    '.ps1', '.psm1', '.sh', '.bash',
    '.yml', '.yaml',
    '.kt', '.kts', '.gradle',
    '.py', '.go',
    '.iss', '.nsi', '.nsh', '.wxs'
)

function Test-ExcludedPath {
    param([string]$RelativePath)

    $normalized = $RelativePath -replace '/', '\'

    foreach ($dir in $excludeDirs) {
        if ($normalized -like "*\$dir\*" -or $normalized -like "$dir\*" -or $normalized -like "*\$dir") {
            return $true
        }
    }

    foreach ($f in $excludeFiles) {
        if ($normalized -ieq $f) {
            return $true
        }
    }

    return $false
}

Write-Host "[check-swap] Сбор файлов..." -ForegroundColor DarkGray

$files = Get-ChildItem -LiteralPath $Root -Recurse -File -Force -ErrorAction SilentlyContinue |
    Where-Object {
        $ext = $_.Extension.ToLowerInvariant()
        if (-not ($textExtensions -contains $ext)) { return $false }
        $rel = $_.FullName.Substring($Root.Length).TrimStart('\', '/')
        return -not (Test-ExcludedPath -RelativePath $rel)
    }

Write-Host "[check-swap] Файлов к проверке: $($files.Count)" -ForegroundColor DarkGray

$findings = New-Object System.Collections.Generic.List[object]

foreach ($entry in $forbiddenPatterns) {
    $pattern = $entry.Pattern
    $reason = $entry.Reason

    $matches = $files | Select-String -Pattern $pattern -CaseSensitive:$false -ErrorAction SilentlyContinue

    foreach ($m in $matches) {
        $rel = $m.Path.Substring($Root.Length).TrimStart('\', '/')
        $findings.Add([pscustomobject]@{
            Pattern = $pattern
            Reason  = $reason
            File    = $rel
            Line    = $m.LineNumber
            Text    = $m.Line.Trim()
        }) | Out-Null
    }
}

if ($findings.Count -eq 0) {
    Write-Host ''
    Write-Host "[check-swap] Чисто. Оставшихся ссылок на старые токены не найдено." -ForegroundColor Green
    exit 0
}

Write-Host ''
Write-Host "[check-swap] Найдено нарушений: $($findings.Count)" -ForegroundColor Red

$grouped = $findings | Group-Object -Property Pattern
foreach ($g in $grouped) {
    $reason = ($g.Group | Select-Object -First 1).Reason
    Write-Host ''
    Write-Host "  Pattern: $($g.Name)" -ForegroundColor Yellow
    Write-Host "  Причина: $reason" -ForegroundColor DarkYellow
    Write-Host "  Совпадений: $($g.Count)" -ForegroundColor DarkYellow
    foreach ($f in $g.Group) {
        if ($ShowContext) {
            Write-Host "    $($f.File):$($f.Line)  $($f.Text)"
        } else {
            Write-Host "    $($f.File):$($f.Line)"
        }
    }
}

exit 1
