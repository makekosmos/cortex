param(
  [switch]$MigrateAutostart,
  [switch]$StartEngine,
  # Overridable only so headless tests can point at a scratch root and Run
  # key instead of real machine state. The installer never passes these.
  [string]$EngineRoot = (Join-Path $env:LOCALAPPDATA 'Mundus\Engine'),
  [string]$RunKeyPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run',
  [string]$StartupApprovedPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run',
  [switch]$DryRun
)

# Post-install steps that must not live as inline -Command strings in
# installer.nsi (NSIS single-quoted strings have no escaping). Resolves the
# installed Engine from %LOCALAPPDATA%\Mundus\Engine\current.json, failing
# closed if the pointer or binary is missing, then optionally:
#   -MigrateAutostart: writes HKCU Run "Mundus Engine" = "<exe>" --start
#       unless a persisted opt-out exists (a StartupApproved disabled
#       marker under the current or any legacy product name)
#   -StartEngine:      launches the Engine hidden without waiting

$ErrorActionPreference = 'Stop'
$semver = '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$'

$pointerFile = Join-Path $EngineRoot 'current.json'
$pointer = Get-Content -Raw -LiteralPath $pointerFile | ConvertFrom-Json
if ($pointer.schema_version -ne 1 -or $pointer.version -notmatch $semver) {
  throw 'invalid current.json'
}
$exe = Join-Path (Join-Path $EngineRoot 'versions') $pointer.version
$exe = Join-Path $exe 'mundus-engine.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
  throw "engine binary missing: $exe"
}
$command = '"' + $exe + '" --start'

# StartupApproved\Run REG_BINARY marker: byte 0 is 2 = enabled and 3 (6 on
# some builds) = explicitly disabled. A Task Manager/Settings disable keeps
# the Run value and writes this marker; `engine.autostart.set 0` writes the
# same layout. It is the only opt-out signal that survives an upgrade —
# an absent Run value proves nothing (the Electron-era 0.9.x only wrote it
# when the user toggled autostart on).
$script:AutostartOptOutNames = @(
  'Mundus Engine',
  # MIGRATION(KOS-267): every prior generation's Run value name — a
  # disabled marker under one means the user opted out before the upgrade.
  'Kosmos Engine', 'Kosmos', 'electron.app.Kosmos', 'com.kazui.kosmos', # MIGRATION(KOS-267)
  'com.kazui.kepler', 'Kepler', 'KeplerKosmos', 'KosmosKepler' # MIGRATION(KOS-267)
)

function Find-AutostartOptOut {
  foreach ($name in $script:AutostartOptOutNames) {
    $item = Get-ItemProperty -LiteralPath $StartupApprovedPath -Name $name -ErrorAction SilentlyContinue
    if ($null -eq $item) { continue }
    $bytes = [byte[]]($item.$name)
    if ($bytes.Length -ge 1 -and $bytes[0] -in @(3, 6)) { return $name }
  }
  return $null
}

if ($MigrateAutostart) {
  $optOut = Find-AutostartOptOut
  if ($optOut) {
    Write-Output "AUTOSTART-SKIPPED opt-out=$optOut"
  } elseif ($DryRun) {
    Write-Output "AUTOSTART $RunKeyPath 'Mundus Engine' = $command"
  } else {
    New-Item -Path $RunKeyPath -Force | Out-Null
    Set-ItemProperty -LiteralPath $RunKeyPath -Name 'Mundus Engine' -Value $command
    # Mark the entry enabled so Task Manager shows the same state — the
    # write the OS itself makes on Enable. The marker also clears a stale
    # enabled-flag ambiguity for the next migration read.
    New-Item -Path $StartupApprovedPath -Force | Out-Null
    $marker = [byte[]](@(0x02, 0, 0, 0) + [BitConverter]::GetBytes([DateTimeOffset]::UtcNow.ToFileTime()))
    Set-ItemProperty -LiteralPath $StartupApprovedPath -Name 'Mundus Engine' -Value $marker -Type Binary
    Write-Output "AUTOSTART $RunKeyPath 'Mundus Engine' = $command"
  }
}
if ($StartEngine) {
  if ($DryRun) {
    Write-Output "START $command"
  } else {
    Start-Process -FilePath $exe -ArgumentList '--start' -WindowStyle Hidden
  }
}
