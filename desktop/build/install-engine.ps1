param(
  [Parameter(Mandatory = $true)][string]$Archive,
  [Parameter(Mandatory = $true)][string]$Manifest,
  [Parameter(Mandatory = $true)][string]$TargetRoot,
  [string]$Url
)

$ErrorActionPreference = 'Stop'
function Get-EngineSha256([string]$Path) {
  $sha = [Security.Cryptography.SHA256]::Create()
  try { return ([BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($Path))) -replace '-', '').ToLowerInvariant() }
  finally { $sha.Dispose() }
}
$expected = Get-Content -Raw -LiteralPath $Manifest | ConvertFrom-Json
if ($expected.schema_version -ne 1 -or $expected.product -ne 'kosmos-engine') { throw 'invalid engine manifest' }
$metadataTemp = Join-Path ([IO.Path]::GetTempPath()) ("kosmos-engine-manifest.$PID.json")
trap { Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $metadataTemp; throw }
if ($expected.channel_url -and -not (Test-Path -LiteralPath $Archive)) {
  if ($expected.channel_url -notmatch '^https://') { throw 'engine metadata URL must use HTTPS' }
  Invoke-WebRequest -Uri $expected.channel_url -OutFile $metadataTemp -UseBasicParsing
  $expected = Get-Content -Raw -LiteralPath $metadataTemp | ConvertFrom-Json
  if ($expected.schema_version -ne 1 -or $expected.product -ne 'kosmos-engine' -or $expected.url -notmatch '^https://') { throw 'invalid latest engine manifest' }
}
if (-not (Test-Path -LiteralPath $Archive)) {
  if ([string]::IsNullOrWhiteSpace($Url)) { $Url = $expected.url }
  if ($Url -notmatch '^https://') { throw 'engine download URL must use HTTPS' }
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Archive) | Out-Null
  Invoke-WebRequest -Uri $Url -OutFile $Archive -UseBasicParsing
}
if ((Get-EngineSha256 $Archive) -ne $expected.archive_sha256.ToLowerInvariant()) { throw 'engine archive hash mismatch' }

$currentFile = Join-Path $TargetRoot 'current.json'
if (Test-Path -LiteralPath $currentFile) {
  try {
    $current = Get-Content -Raw -LiteralPath $currentFile | ConvertFrom-Json
    $currentRoot = Join-Path (Join-Path $TargetRoot 'versions') $current.version
    $valid = $current.schema_version -eq 1 -and $current.version -eq $expected.version
    foreach ($file in $expected.files) {
      $candidate = Join-Path $currentRoot $file.name
      if (-not (Test-Path -LiteralPath $candidate) -or (Get-Item -LiteralPath $candidate).Length -ne $file.size -or (Get-EngineSha256 $candidate) -ne $file.sha256.ToLowerInvariant()) { $valid = $false; break }
    }
    if ($valid) { exit 0 }
  } catch { }
}

$temp = Join-Path (Join-Path $TargetRoot 'versions') ("$($expected.version).$PID.tmp")
$versionRoot = Join-Path (Join-Path $TargetRoot 'versions') $expected.version
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue -LiteralPath $temp
New-Item -ItemType Directory -Force -Path $temp | Out-Null
Expand-Archive -LiteralPath $Archive -DestinationPath $temp -Force
foreach ($file in $expected.files) {
  $candidate = Join-Path $temp $file.name
  if (-not (Test-Path -LiteralPath $candidate)) { throw "engine file missing: $($file.name)" }
  $actual = Get-Item -LiteralPath $candidate
  if ($actual.Length -ne $file.size -or (Get-EngineSha256 $candidate) -ne $file.sha256.ToLowerInvariant()) { throw "engine file mismatch: $($file.name)" }
}
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $versionRoot) | Out-Null
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue -LiteralPath $versionRoot
Move-Item -LiteralPath $temp -Destination $versionRoot
$pointerTemp = "$currentFile.$PID.tmp"
Set-Content -LiteralPath $pointerTemp -Value (@{ schema_version = 1; version = $expected.version } | ConvertTo-Json -Compress) -Encoding ASCII
Move-Item -Force -LiteralPath $pointerTemp -Destination $currentFile
Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $metadataTemp
