param(
  [switch]$SeedAutostart,
  [switch]$StartEngine,
  # Overridable only so headless tests can point at a scratch root and Run
  # key instead of real machine state. The installer never passes these.
  [string]$EngineRoot = (Join-Path $env:LOCALAPPDATA 'Kosmos\Engine'),
  [string]$RunKeyPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run',
  [switch]$DryRun
)

# KOS-236: post-install steps that must not live as inline -Command strings in
# installer.nsi (NSIS single-quoted strings have no escaping). Resolves the
# installed Engine from %LOCALAPPDATA%\Kosmos\Engine\current.json, failing
# closed if the pointer or binary is missing, then optionally:
#   -SeedAutostart: writes HKCU Run "Kosmos Engine" = "<exe>" --start
#   -StartEngine:   launches the Engine hidden without waiting

$ErrorActionPreference = 'Stop'
$semver = '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$'

$pointerFile = Join-Path $EngineRoot 'current.json'
$pointer = Get-Content -Raw -LiteralPath $pointerFile | ConvertFrom-Json
if ($pointer.schema_version -ne 1 -or $pointer.version -notmatch $semver) {
  throw 'invalid current.json'
}
$exe = Join-Path (Join-Path $EngineRoot 'versions') $pointer.version
$exe = Join-Path $exe 'kepler-backend.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
  throw "engine binary missing: $exe"
}
$command = '"' + $exe + '" --start'

if ($SeedAutostart) {
  if ($DryRun) {
    Write-Output "AUTOSTART $RunKeyPath 'Kosmos Engine' = $command"
  } else {
    Set-ItemProperty -LiteralPath $RunKeyPath -Name 'Kosmos Engine' -Value $command
  }
}
if ($StartEngine) {
  if ($DryRun) {
    Write-Output "START $command"
  } else {
    Start-Process -FilePath $exe -ArgumentList '--start' -WindowStyle Hidden
  }
}
