param(
  [string]$Archive,
  [string]$Manifest,
  [Parameter(Mandatory = $true)][string]$TargetRoot,
  [string]$Url,
  [switch]$Uninstall,
  [string]$Version
)

$ErrorActionPreference = 'Stop'
function Get-EngineSha256([string]$Path) {
  $sha = [Security.Cryptography.SHA256]::Create()
  try { return ([BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($Path))) -replace '-', '').ToLowerInvariant() }
  finally { $sha.Dispose() }
}
function Test-TrustedReleaseUrl([string]$Value) {
  return $Value -match '^https://github\.com/makekosmos/desktop/releases/(latest/download/|download/v[^/]+/)'
}
function Test-EngineProcess([string]$Path) {
  $fullPath = [IO.Path]::GetFullPath($Path)
  foreach ($process in @(Get-CimInstance Win32_Process -Filter "Name='kepler-backend.exe'" -ErrorAction Stop)) {
    if (-not $process.ExecutablePath) { throw 'cannot determine the running Engine path' }
    if ([IO.Path]::GetFullPath($process.ExecutablePath) -ieq $fullPath) { return $true }
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
    $defaultRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Kosmos\Engine')).TrimEnd('\')
    if ([IO.Path]::GetFullPath($TargetRoot).TrimEnd('\') -ine $defaultRoot) { return }
    $runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    $run = Get-ItemProperty -LiteralPath $runKey -Name 'Kosmos Engine' -ErrorAction Stop
    if ([string]$run.'Kosmos Engine' -match '(?i)(?:kepler-backend|Kosmos Runtime)\.exe.*--start') {
      Set-ItemProperty -LiteralPath $runKey -Name 'Kosmos Engine' -Value ('"' + (Join-Path $VersionRoot 'kepler-backend.exe') + '" --start')
    }
  } catch { }
}
if ($Uninstall) {
  if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'invalid Engine uninstall version' }
  $root = [IO.Path]::GetFullPath($TargetRoot).TrimEnd('\')
  $versionsRoot = [IO.Path]::GetFullPath((Join-Path $root 'versions')).TrimEnd('\')
  $versionRoot = [IO.Path]::GetFullPath((Join-Path $versionsRoot $Version)).TrimEnd('\')
  if (-not $versionRoot.StartsWith("$versionsRoot\", [StringComparison]::OrdinalIgnoreCase)) { throw 'invalid Engine uninstall path' }
  if (-not (Test-Path -LiteralPath $versionRoot -PathType Container)) { exit 2 }
  $knownFiles = @('kepler-backend.exe', 'ark-core-rpc.exe', 'kepler-focus-helper.exe', 'kepler-focus-svc.exe', 'engine-manifest.json')
  foreach ($name in $knownFiles) {
    $candidate = Join-Path $versionRoot $name
    if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) { throw "Engine file missing: $name" }
  }
  # Newer payloads add files the strict check above predates; drop them so the
  # version directory empties cleanly.
  $optionalFiles = @('tray.ico')
  $currentFile = Join-Path $root 'current.json'
  $currentVersion = $null
  if (Test-Path -LiteralPath $currentFile) {
    try { $currentVersion = (Get-Content -Raw -LiteralPath $currentFile | ConvertFrom-Json).version } catch { }
  }
  $wasRunning = Stop-EngineForReplacement (Join-Path $versionRoot 'kepler-backend.exe')
  foreach ($name in $knownFiles) { Remove-Item -Force -LiteralPath (Join-Path $versionRoot $name) -ErrorAction Stop }
  foreach ($name in $optionalFiles) { Remove-Item -Force -LiteralPath (Join-Path $versionRoot $name) -ErrorAction SilentlyContinue }
  Remove-Item -LiteralPath $versionRoot -Force -ErrorAction Stop
  if ($currentVersion -eq $Version) { Remove-Item -Force -LiteralPath $currentFile -ErrorAction Stop; exit 0 }
  exit 2
}
function Download-EngineArchive([string]$DownloadUrl) {
  if ([string]::IsNullOrWhiteSpace($DownloadUrl)) { $DownloadUrl = $expected.url }
  if (-not (Test-TrustedReleaseUrl $DownloadUrl)) { throw 'engine download URL is not the trusted release publisher' }
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Archive) | Out-Null
  Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $archiveDownloadTemp
  Invoke-WebRequest -Uri $DownloadUrl -OutFile $archiveDownloadTemp -UseBasicParsing
  if (-not (Test-Path -LiteralPath $archiveDownloadTemp -PathType Leaf)) { throw 'engine download did not produce an archive' }
  if ((Get-EngineSha256 $archiveDownloadTemp) -ne $expected.archive_sha256.ToLowerInvariant()) {
    Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $archiveDownloadTemp
    throw 'engine archive hash mismatch after trusted download'
  }
  Move-Item -Force -LiteralPath $archiveDownloadTemp -Destination $Archive
}
$expected = Get-Content -Raw -LiteralPath $Manifest | ConvertFrom-Json
if ($expected.schema_version -ne 1 -or $expected.product -ne 'kosmos-engine') { throw 'invalid engine manifest' }
if ($expected.version -notmatch '^\d+\.\d+\.\d+$') { throw 'invalid engine version' }
if (-not $expected.files -or @($expected.files).Count -eq 0) { throw 'engine manifest files are required' }
foreach ($file in @($expected.files)) {
  if ($file.name -notmatch '^[A-Za-z0-9._-]+$' -or $file.name -in @('.', '..') -or $file.sha256 -notmatch '^[0-9a-fA-F]{64}$' -or (($file.size -isnot [int]) -and ($file.size -isnot [long])) -or $file.size -lt 0) { throw 'invalid engine manifest file' }
}
if (-not (Test-TrustedReleaseUrl $expected.url)) { throw 'engine archive URL is not the trusted release publisher' }
$metadataTemp = Join-Path ([IO.Path]::GetTempPath()) ("kosmos-engine-manifest.$PID.json")
$archiveDownloadTemp = "$Archive.download.$PID.tmp"
trap {
  Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $metadataTemp
  Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $archiveDownloadTemp
  throw
}
if ($expected.channel_url -and -not (Test-Path -LiteralPath $Archive)) {
  if (-not (Test-TrustedReleaseUrl $expected.channel_url) -or $expected.channel_url -notmatch '/latest/download/Kosmos-Engine-manifest\.json$') { throw 'engine metadata URL is not the trusted release channel' }
  Invoke-WebRequest -Uri $expected.channel_url -OutFile $metadataTemp -UseBasicParsing
  $expected = Get-Content -Raw -LiteralPath $metadataTemp | ConvertFrom-Json
  if ($expected.schema_version -ne 1 -or $expected.product -ne 'kosmos-engine' -or -not (Test-TrustedReleaseUrl $expected.url)) { throw 'invalid latest engine manifest' }
  if ($expected.version -notmatch '^\d+\.\d+\.\d+$' -or -not $expected.files -or @($expected.files).Count -eq 0) { throw 'invalid latest engine manifest' }
  foreach ($file in @($expected.files)) {
    if ($file.name -notmatch '^[A-Za-z0-9._-]+$' -or $file.name -in @('.', '..') -or $file.sha256 -notmatch '^[0-9a-fA-F]{64}$' -or (($file.size -isnot [int]) -and ($file.size -isnot [long])) -or $file.size -lt 0) { throw 'invalid latest engine manifest' }
  }
}

$currentFile = Join-Path $TargetRoot 'current.json'
if (Test-Path -LiteralPath $currentFile) {
  try {
    $current = Get-Content -Raw -LiteralPath $currentFile | ConvertFrom-Json
    $valid = $current.schema_version -eq 1 -and $current.version -eq $expected.version
    if ($valid) {
      $currentRoot = Join-Path (Join-Path $TargetRoot 'versions') $current.version
      foreach ($file in $expected.files) {
        $candidate = Join-Path $currentRoot $file.name
        if (-not (Test-Path -LiteralPath $candidate) -or (Get-Item -LiteralPath $candidate).Length -ne $file.size -or (Get-EngineSha256 $candidate) -ne $file.sha256.ToLowerInvariant()) { $valid = $false; break }
      }
    }
    if ($valid) { Update-EngineAutostart $currentRoot; exit 0 }
  } catch { }
}

if (-not (Test-Path -LiteralPath $Archive -PathType Leaf)) {
  if (Test-Path -LiteralPath $Archive) { throw 'engine archive path is not a file' }
  Download-EngineArchive $Url
}
$archiveHash = Get-EngineSha256 $Archive
if ($archiveHash -ne $expected.archive_sha256.ToLowerInvariant()) {
  Download-EngineArchive $Url
}

$temp = Join-Path (Join-Path $TargetRoot 'versions') ("$($expected.version).$PID.tmp")
$versionRoot = Join-Path (Join-Path $TargetRoot 'versions') $expected.version
$versionsRoot = [IO.Path]::GetFullPath((Join-Path $TargetRoot 'versions')).TrimEnd('\')
$temp = [IO.Path]::GetFullPath($temp)
$versionRoot = [IO.Path]::GetFullPath($versionRoot)
if (-not $temp.StartsWith("$versionsRoot\", [StringComparison]::OrdinalIgnoreCase) -or
    -not $versionRoot.StartsWith("$versionsRoot\", [StringComparison]::OrdinalIgnoreCase)) {
  throw 'invalid Engine installation path'
}
$engineWasRunning = $false
$currentEnginePath = $null
if (Test-Path -LiteralPath $currentFile) {
  try {
    $currentPointer = Get-Content -Raw -LiteralPath $currentFile | ConvertFrom-Json
    if ($currentPointer.schema_version -eq 1 -and $currentPointer.version -match '^\d+\.\d+\.\d+$') {
      $currentEnginePath = Join-Path (Join-Path $TargetRoot 'versions') $currentPointer.version
      $currentEnginePath = Join-Path $currentEnginePath 'kepler-backend.exe'
    }
  } catch { }
}
if ($currentEnginePath -and (Test-Path -LiteralPath $currentEnginePath)) {
  $engineWasRunning = Stop-EngineForReplacement $currentEnginePath
}
if (Test-Path -LiteralPath $versionRoot) {
  $newEnginePath = Join-Path $versionRoot 'kepler-backend.exe'
  if (-not $currentEnginePath -or [IO.Path]::GetFullPath($currentEnginePath) -ine [IO.Path]::GetFullPath($newEnginePath)) {
    $engineWasRunning = (Stop-EngineForReplacement $newEnginePath) -or $engineWasRunning
  }
}
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
if (Test-Path -LiteralPath $versionRoot) { throw 'Engine version directory could not be replaced' }
Move-Item -LiteralPath $temp -Destination $versionRoot
foreach ($file in $expected.files) {
  $candidate = Join-Path $versionRoot $file.name
  if (-not (Test-Path -LiteralPath $candidate -PathType Leaf) -or
      (Get-Item -LiteralPath $candidate).Length -ne $file.size -or
      (Get-EngineSha256 $candidate) -ne $file.sha256.ToLowerInvariant()) { throw "installed Engine file mismatch: $($file.name)" }
}
$pointerTemp = "$currentFile.$PID.tmp"
Set-Content -LiteralPath $pointerTemp -Value (@{ schema_version = 1; version = $expected.version } | ConvertTo-Json -Compress) -Encoding ASCII
Move-Item -Force -LiteralPath $pointerTemp -Destination $currentFile
if ($engineWasRunning) {
  try {
    Start-Process -FilePath (Join-Path $versionRoot 'kepler-backend.exe') -ArgumentList '--start' -WindowStyle Hidden
  } catch {
    Write-Warning "Engine was replaced but could not be restarted: $($_.Exception.Message)"
  }
}
Update-EngineAutostart $versionRoot
Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $metadataTemp
