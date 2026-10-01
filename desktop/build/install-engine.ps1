param(
  [Parameter(Mandatory = $true)][string]$Archive,
  [Parameter(Mandatory = $true)][string]$Manifest,
  [Parameter(Mandatory = $true)][string]$TargetRoot,
  # Overridable only so headless tests can point migration at a scratch
  # registry key/shortcut instead of the real machine state. Production
  # (installer.nsi) never passes these — it always migrates the real
  # standalone "Kosmos Engine" registration.
  # MIGRATION(KOS-267): remove after 2026-11-01.
  [string]$LegacyRegistryKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine', # MIGRATION(KOS-267)
  [string]$LegacyShortcut
)

# KOS-233: one product, one version. Mundus Desktop ships the Engine it was
# built with (bundled locally by build-backend.mjs into $Archive/$Manifest —
# never downloaded from a published release). This script:
#   1. Installs that Engine into the shared %LOCALAPPDATA%\Mundus\Engine root
#      (also used by Manager/Agenda/Memoria), never downgrading a newer,
#      already-verified installation left by a later Desktop version.
#   2. Takes over an existing standalone "Kosmos Engine" installation (the
#      old separate installer's Apps & Features entry): removes its
#      registration and Start Menu shortcut, keeps its data/autostart
#      preference. Snapshots before changing anything and restores on
#      failure. MIGRATION(KOS-267): remove after 2026-11-01.

$ErrorActionPreference = 'Stop'
$semver = '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$'

# Engine binaries from every product generation, oldest first.
# MIGRATION(KOS-267): remove the legacy names after 2026-11-01.
$script:EngineBinaryNames = @('mundus-engine.exe', 'kepler-backend.exe') # MIGRATION(KOS-267)

function Get-EngineSha256([string]$Path) {
  $sha = [Security.Cryptography.SHA256]::Create()
  try { return ([BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($Path))) -replace '-', '').ToLowerInvariant() }
  finally { $sha.Dispose() }
}

function Compare-EngineVersion([string]$Left, [string]$Right) {
  ([version]$Left).CompareTo([version]$Right)
}

function Assert-Manifest([object]$Value) {
  # MIGRATION(KOS-267): 'kosmos-engine' is the product id written by
  # MIGRATION(KOS-267): Kosmos-era manifests (installed state on user machines).
  if ($Value.schema_version -ne 1 -or
      $Value.product -notin @('mundus-engine', 'kosmos-engine') -or # MIGRATION(KOS-267)
      $Value.version -notmatch $semver) { throw 'invalid engine manifest' }
  if (-not $Value.files -or @($Value.files).Count -eq 0) { throw 'engine manifest files are required' }
  foreach ($file in @($Value.files)) {
    if ($file.name -notmatch '^[A-Za-z0-9._-]+$' -or $file.name -in @('.', '..') -or
        $file.sha256 -notmatch '^[0-9a-fA-F]{64}$' -or
        (($file.size -isnot [int]) -and ($file.size -isnot [long])) -or $file.size -lt 0) {
      throw 'invalid engine manifest file'
    }
  }
}

# Returns the installed+verified version at $Root, or $null.
function Test-InstalledEngine([string]$Root) {
  try {
    $currentFile = Join-Path $Root 'current.json'
    if (-not (Test-Path -LiteralPath $currentFile)) { return $null }
    $pointer = Get-Content -Raw -LiteralPath $currentFile | ConvertFrom-Json
    if ($pointer.schema_version -ne 1 -or $pointer.version -notmatch $semver) { return $null }
    $versionRoot = Join-Path (Join-Path $Root 'versions') $pointer.version
    $installedManifest = Get-Content -Raw -LiteralPath (Join-Path $versionRoot 'engine-manifest.json') | ConvertFrom-Json
    Assert-Manifest $installedManifest
    if ($installedManifest.version -ne $pointer.version) { return $null }
    foreach ($file in @($installedManifest.files)) {
      $candidate = Join-Path $versionRoot $file.name
      if (-not (Test-Path -LiteralPath $candidate -PathType Leaf) -or
          (Get-Item -LiteralPath $candidate).Length -ne $file.size -or
          (Get-EngineSha256 $candidate) -ne $file.sha256.ToLowerInvariant()) { return $null }
    }
    return $pointer.version
  } catch { return $null }
}

# True when the verified installation at $Version is byte-for-byte the build
# described by $Expected (same file names and hashes). A same-version rebuild
# (local builds, a re-cut release) differs here and must replace it.
function Test-SameEngineBuild([string]$Root, [string]$Version, [object]$Expected) {
  try {
    $installed = Get-Content -Raw -LiteralPath (Join-Path (Join-Path (Join-Path $Root 'versions') $Version) 'engine-manifest.json') | ConvertFrom-Json
    $key = { param($files) (@($files) | ForEach-Object { "$($_.name)=$($_.sha256.ToLowerInvariant())" } | Sort-Object) -join ';' }
    return (& $key $installed.files) -eq (& $key $Expected.files)
  } catch { return $false }
}

function Test-EngineProcess([string]$Path) {
  $fullPath = [IO.Path]::GetFullPath($Path)
  # MIGRATION(KOS-267): the running pre-upgrade Engine is still the old binary
  # name; match both.
  foreach ($name in $script:EngineBinaryNames) {
    foreach ($process in @(Get-CimInstance Win32_Process -Filter "Name='$name'" -ErrorAction Stop)) {
      if (-not $process.ExecutablePath) { throw 'cannot determine the running Engine path' }
      if ([IO.Path]::GetFullPath($process.ExecutablePath) -ieq $fullPath) { return $true }
    }
  }
  return $false
}
function Stop-EngineForReplacement([string]$Path) {
  if (-not (Test-EngineProcess $Path)) { return $false }
  $result = Start-Process -FilePath $Path -ArgumentList '--shutdown' -PassThru -WindowStyle Hidden
  $result.WaitForExit()
  Start-Sleep -Milliseconds 750
  if (Test-EngineProcess $Path) { throw 'running Engine did not stop before replacement' }
  return $true
}
function Update-EngineAutostart([string]$VersionRoot) {
  try {
    $defaultRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Mundus\Engine')).TrimEnd('\')
    if ([IO.Path]::GetFullPath($TargetRoot).TrimEnd('\') -ine $defaultRoot) { return }
    $runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    $exe = Join-Path $VersionRoot 'mundus-engine.exe'
    # MIGRATION(KOS-267): keep reading/rewriting the legacy Run value names
    # and binary names until cleanup; always write under the new name.
    foreach ($name in @('Mundus Engine', 'Kosmos Engine')) { # MIGRATION(KOS-267)
      $run = Get-ItemProperty -LiteralPath $runKey -Name $name -ErrorAction SilentlyContinue
      if ($run -and [string]$run.$name -match '(?i)(?:kepler-backend|mundus-engine|Kosmos Runtime)\.exe.*--start') { # MIGRATION(KOS-267)
        Set-ItemProperty -LiteralPath $runKey -Name 'Mundus Engine' -Value ('"' + $exe + '" --start')
      }
      if ($name -ne 'Mundus Engine') {
        Remove-ItemProperty -LiteralPath $runKey -Name $name -ErrorAction SilentlyContinue
      }
    }
  } catch { }
}

# Takes over an existing standalone "Kosmos Engine" Apps&Features entry left
# by the old separate installer: removes its registration and shortcut, but
# never touches %APPDATA%\Kosmos (user data — the Engine moves it on first
# start) or the LOCALAPPDATA Engine files themselves.
# MIGRATION(KOS-267): remove after 2026-11-01.
function Invoke-EngineMigration {
  $key = $LegacyRegistryKey
  if (-not (Test-Path -LiteralPath $key)) { return }
  $shortcut = if ($LegacyShortcut) { $LegacyShortcut } else { Join-Path ([Environment]::GetFolderPath('Programs')) 'Kosmos Engine.lnk' } # MIGRATION(KOS-267)
  $props = Get-ItemProperty -LiteralPath $key
  $snapshot = @{}
  foreach ($name in @('DisplayName', 'DisplayVersion', 'Publisher', 'InstallLocation', 'UninstallString', 'QuietUninstallString', 'DisplayIcon', 'NoModify', 'NoRepair')) {
    if ($props.PSObject.Properties.Name -contains $name) { $snapshot[$name] = $props.$name }
  }
  $shortcutBackup = $null
  if (Test-Path -LiteralPath $shortcut -PathType Leaf) {
    $shortcutBackup = "$env:TEMP\mundus-engine-shortcut-$PID.bak"
    Copy-Item -LiteralPath $shortcut -Destination $shortcutBackup -Force
  }
  try {
    Remove-Item -LiteralPath $key -Recurse -Force -ErrorAction Stop
    if ($shortcutBackup) { Remove-Item -LiteralPath $shortcut -Force -ErrorAction Stop }
    if ($snapshot.InstallLocation) {
      # Best-effort: an orphaned standalone Uninstall.exe is no longer
      # reachable from Apps & Features, but leaving it around is dead weight.
      Remove-Item -LiteralPath (Join-Path $snapshot.InstallLocation 'Uninstall.exe') -Force -ErrorAction SilentlyContinue
    }
  } catch {
    New-Item -Path $key -Force | Out-Null
    foreach ($entry in $snapshot.GetEnumerator()) {
      $type = if ($entry.Key -in @('NoModify', 'NoRepair')) { 'DWord' } else { 'String' }
      New-ItemProperty -LiteralPath $key -Name $entry.Key -Value $entry.Value -PropertyType $type -Force | Out-Null
    }
    if ($shortcutBackup -and (Test-Path -LiteralPath $shortcutBackup)) {
      Copy-Item -LiteralPath $shortcutBackup -Destination $shortcut -Force
    }
    throw "Kosmos Engine migration failed and was rolled back: $($_.Exception.Message)" # MIGRATION(KOS-267)
  } finally {
    if ($shortcutBackup) { Remove-Item -LiteralPath $shortcutBackup -Force -ErrorAction SilentlyContinue }
  }
}

$expected = Get-Content -Raw -LiteralPath $Manifest | ConvertFrom-Json
Assert-Manifest $expected
if (-not (Test-Path -LiteralPath $Archive -PathType Leaf)) { throw "engine archive missing: $Archive" }
if ($expected.archive_sha256 -and (Get-EngineSha256 $Archive) -ne $expected.archive_sha256.ToLowerInvariant()) {
  throw 'engine archive hash mismatch'
}

$installedVersion = Test-InstalledEngine $TargetRoot
$installedOrder = if ($installedVersion) { Compare-EngineVersion $installedVersion $expected.version } else { $null }
if ($installedVersion -and ($installedOrder -gt 0 -or
    ($installedOrder -eq 0 -and (Test-SameEngineBuild $TargetRoot $installedVersion $expected)))) {
  # Monotonic: never replace a newer, already-verified Engine, nor the very
  # same build. An equal version with different bytes is replaced below.
  Update-EngineAutostart (Join-Path (Join-Path $TargetRoot 'versions') $installedVersion)
} else {
  $currentFile = Join-Path $TargetRoot 'current.json'
  $versionsRoot = [IO.Path]::GetFullPath((Join-Path $TargetRoot 'versions')).TrimEnd('\')
  $versionRoot = [IO.Path]::GetFullPath((Join-Path $versionsRoot $expected.version))
  $temp = [IO.Path]::GetFullPath((Join-Path $versionsRoot "$($expected.version).$PID.tmp"))
  if (-not $temp.StartsWith("$versionsRoot\", [StringComparison]::OrdinalIgnoreCase) -or
      -not $versionRoot.StartsWith("$versionsRoot\", [StringComparison]::OrdinalIgnoreCase)) {
    throw 'invalid Engine installation path'
  }

  $engineWasRunning = $false
  if ($installedVersion) {
    $installedPath = Join-Path (Join-Path $TargetRoot 'versions') $installedVersion
    foreach ($name in $script:EngineBinaryNames) {
      $engineWasRunning = (Stop-EngineForReplacement (Join-Path $installedPath $name)) -or $engineWasRunning
    }
  }
  if (Test-Path -LiteralPath $versionRoot) {
    foreach ($name in $script:EngineBinaryNames) {
      $engineWasRunning = (Stop-EngineForReplacement (Join-Path $versionRoot $name)) -or $engineWasRunning
    }
  }
  # Pre-in-process Engines spawned an ark-core-rpc sidecar that survives a
  # backend kill and still holds the DB open; stop any leftover before
  # replacing files.
  Get-Process -Name 'ark-core-rpc' -ErrorAction SilentlyContinue | Stop-Process -Force

  if (Test-Path -LiteralPath $temp) { Remove-Item -Recurse -Force -ErrorAction Stop -LiteralPath $temp }
  New-Item -ItemType Directory -Force -Path $temp | Out-Null
  Expand-Archive -LiteralPath $Archive -DestinationPath $temp -Force
  foreach ($file in $expected.files) {
    $candidate = Join-Path $temp $file.name
    if (-not (Test-Path -LiteralPath $candidate)) { throw "engine file missing: $($file.name)" }
    $actual = Get-Item -LiteralPath $candidate
    if ($actual.Length -ne $file.size -or (Get-EngineSha256 $candidate) -ne $file.sha256.ToLowerInvariant()) { throw "engine file mismatch: $($file.name)" }
  }
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $versionRoot) | Out-Null
  if (Test-Path -LiteralPath $versionRoot) { Remove-Item -Recurse -Force -ErrorAction Stop -LiteralPath $versionRoot }
  Move-Item -LiteralPath $temp -Destination $versionRoot
  foreach ($file in $expected.files) {
    $candidate = Join-Path $versionRoot $file.name
    if (-not (Test-Path -LiteralPath $candidate -PathType Leaf) -or
        (Get-Item -LiteralPath $candidate).Length -ne $file.size -or
        (Get-EngineSha256 $candidate) -ne $file.sha256.ToLowerInvariant()) { throw "installed Engine file mismatch: $($file.name)" }
  }

  # Monotonic guard on the pointer itself: a concurrent install of a newer
  # version (e.g. two Desktop installers racing) must not be clobbered by
  # this one finishing second.
  $pointerVersion = Test-InstalledEngine $TargetRoot
  if (-not $pointerVersion -or (Compare-EngineVersion $expected.version $pointerVersion) -gt 0) {
    $pointerTemp = "$currentFile.$PID.tmp"
    Set-Content -LiteralPath $pointerTemp -Value (@{ schema_version = 1; version = $expected.version } | ConvertTo-Json -Compress) -Encoding ASCII
    Move-Item -Force -LiteralPath $pointerTemp -Destination $currentFile
  }
  if ($engineWasRunning) {
    try {
      Start-Process -FilePath (Join-Path $versionRoot 'mundus-engine.exe') -ArgumentList '--start' -WindowStyle Hidden
    } catch {
      Write-Warning "Engine was replaced but could not be restarted: $($_.Exception.Message)"
    }
  }
  Update-EngineAutostart $versionRoot

  # KOS-261: prune superseded versions/<v> dirs — keep current + one previous
  # for rollback. The selection rule lives in the Engine itself (the same code
  # runs at every Engine startup), so call the just-verified exe rather than
  # duplicating it here. Only the exe installed by this script is invoked: it
  # is guaranteed to carry the subcommand, while an unrelated binary in the
  # skip branch above might not. Best-effort — a failed prune is a warning,
  # never a failed install; the next Engine start retries the same rule.
  try {
    & (Join-Path $versionRoot 'mundus-engine.exe') 'prune-versions' | Out-Null
  } catch {
    Write-Warning "engine versions prune failed: $($_.Exception.Message)"
  }
}

Invoke-EngineMigration
