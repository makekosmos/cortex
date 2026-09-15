param(
  [Parameter(Mandatory = $true)][string]$Manifest,
  [Parameter(Mandatory = $true)][string]$TargetRoot,
  [string]$InstallerPath
)

$ErrorActionPreference = 'Stop'
$semver = '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$'
$engineKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine'
$engineFiles = @('kepler-backend.exe', 'ark-core-rpc.exe', 'kepler-focus-helper.exe', 'kepler-focus-svc.exe')
# Files newer Engine payloads may add; older installed manifests stay valid.
$engineOptionalFiles = @('tray.ico')

function Get-FileSha256([string]$Path) {
  $sha = [Security.Cryptography.SHA256]::Create()
  try { return ([BitConverter]::ToString($sha.ComputeHash([IO.File]::ReadAllBytes($Path))) -replace '-', '').ToLowerInvariant() }
  finally { $sha.Dispose() }
}

function Compare-EngineVersion([string]$Left, [string]$Right) {
  ([version]$Left).CompareTo([version]$Right)
}

function Assert-Manifest([object]$Value, [bool]$RequireInstaller = $true) {
  if ($Value.schema_version -ne 1 -or $Value.product -ne 'kosmos-engine' -or
      $Value.version -notmatch $semver) { throw 'invalid engine manifest' }
  $trustedRelease = "https://github.com/makekosmos/desktop/releases/download/v$($Value.version)/"
  if ($Value.url -cne "${trustedRelease}Kosmos-Engine-$($Value.version).zip") {
    throw 'engine archive URL is not the trusted release publisher'
  }
  if ($RequireInstaller) {
    if ($Value.installer_url -cne "${trustedRelease}Kosmos-Engine-Setup-$($Value.version).exe") {
      throw 'engine installer URL is not the trusted release publisher'
    }
    if ($Value.installer_sha256 -notmatch '^[0-9a-fA-F]{64}$' -or
        (($Value.installer_size -isnot [int]) -and ($Value.installer_size -isnot [long])) -or
        $Value.installer_size -lt 1) { throw 'invalid engine installer metadata' }
  }
  if (-not $Value.files -or @($Value.files).Count -eq 0) {
    throw 'engine manifest files are required'
  }
  foreach ($file in @($Value.files)) {
    if ($file.name -notmatch '^[A-Za-z0-9._-]+$' -or $file.name -in @('.', '..') -or
        $file.sha256 -notmatch '^[0-9a-fA-F]{64}$' -or
        (($file.size -isnot [int]) -and ($file.size -isnot [long])) -or $file.size -lt 0) {
      throw 'invalid engine manifest file'
    }
  }
  $actualNames = @($Value.files | ForEach-Object { $_.name } | Sort-Object)
  $requiredNames = @($engineFiles | Sort-Object)
  $allowedNames = @($engineFiles + $engineOptionalFiles | Sort-Object)
  $missingNames = @($requiredNames | Where-Object { $actualNames -cnotcontains $_ })
  $unexpectedNames = @($actualNames | Where-Object { $allowedNames -cnotcontains $_ })
  if ($missingNames.Count -gt 0 -or $unexpectedNames.Count -gt 0) {
    throw 'engine manifest files are incomplete'
  }
}

function Test-InstalledEngine([string]$Root, [string]$MinimumVersion, [object]$ExpectedManifest) {
  try {
    $pointer = Get-Content -Raw -LiteralPath (Join-Path $Root 'current.json') | ConvertFrom-Json
    if ($pointer.schema_version -ne 1 -or $pointer.version -notmatch $semver -or
        (Compare-EngineVersion $pointer.version $MinimumVersion) -lt 0) { return $false }
    $versionRoot = Join-Path (Join-Path $Root 'versions') $pointer.version
    $installedManifest = Get-Content -Raw -LiteralPath (Join-Path $versionRoot 'engine-manifest.json') | ConvertFrom-Json
    Assert-Manifest $installedManifest $false
    if ($installedManifest.version -ne $pointer.version) { return $false }
    $verificationManifest = if ($ExpectedManifest -and $pointer.version -eq $ExpectedManifest.version) {
      $ExpectedManifest
    } else { $installedManifest }
    foreach ($file in @($verificationManifest.files)) {
      $candidate = Join-Path $versionRoot $file.name
      if (-not (Test-Path -LiteralPath $candidate -PathType Leaf) -or
          (Get-Item -LiteralPath $candidate).Length -ne $file.size -or
          (Get-FileSha256 $candidate) -ne $file.sha256.ToLowerInvariant()) { return $false }
    }
    return $true
  } catch { return $false }
}

$expected = Get-Content -Raw -LiteralPath $Manifest | ConvertFrom-Json
Assert-Manifest $expected
$targetFull = [IO.Path]::GetFullPath($TargetRoot).TrimEnd('\')

# A registry entry is only reusable after its pointer and every declared file pass validation.
try {
  $registered = Get-ItemProperty -LiteralPath $engineKey
  $registeredRoot = [IO.Path]::GetFullPath([string]$registered.InstallLocation).TrimEnd('\')
  $registeredPointer = Get-Content -Raw -LiteralPath (Join-Path $registeredRoot 'current.json') | ConvertFrom-Json
  if ($registered.DisplayName -eq 'Kosmos Engine' -and
      $registered.DisplayVersion -match $semver -and
      $registeredRoot -ieq $targetFull -and
      (Compare-EngineVersion $registered.DisplayVersion $expected.version) -ge 0 -and
      $registered.DisplayVersion -ceq $registeredPointer.version -and
      (Test-InstalledEngine $registeredRoot $expected.version $expected)) { exit 0 }
} catch { }

$tempInstaller = Join-Path ([IO.Path]::GetTempPath()) ("kosmos-engine-$([guid]::NewGuid().ToString('N')).exe")
try {
  if ($InstallerPath) {
    Copy-Item -LiteralPath $InstallerPath -Destination $tempInstaller -Force
  } else {
    if ($expected.installer_url -cne "https://github.com/makekosmos/desktop/releases/download/v$($expected.version)/Kosmos-Engine-Setup-$($expected.version).exe") {
      throw 'engine installer URL is not the trusted release publisher'
    }
    Invoke-WebRequest -Uri $expected.installer_url -OutFile $tempInstaller -UseBasicParsing
  }
  if (-not (Test-Path -LiteralPath $tempInstaller -PathType Leaf) -or
      (Get-Item -LiteralPath $tempInstaller).Length -ne $expected.installer_size -or
      (Get-FileSha256 $tempInstaller) -ne $expected.installer_sha256.ToLowerInvariant()) {
    throw 'engine installer hash or size mismatch'
  }
  $process = Start-Process -FilePath $tempInstaller -ArgumentList '/S' -PassThru -WindowStyle Hidden
  $process.WaitForExit()
  if ($process.ExitCode -ne 0) { throw "engine installer failed with exit code $($process.ExitCode)" }
  try {
    $registered = Get-ItemProperty -LiteralPath $engineKey
    $registeredRoot = [IO.Path]::GetFullPath([string]$registered.InstallLocation).TrimEnd('\')
    $installedPointer = Get-Content -Raw -LiteralPath (Join-Path $registeredRoot 'current.json') | ConvertFrom-Json
    if ($registered.DisplayName -ne 'Kosmos Engine' -or $registeredRoot -ine $targetFull -or
        $registered.DisplayVersion -cne $installedPointer.version -or
        -not (Test-InstalledEngine $registeredRoot $expected.version $expected)) { throw 'engine installer did not register a valid installation' }
  } catch { throw 'engine installer did not register a valid installation' }
} finally {
  Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $tempInstaller
}
