$ErrorActionPreference = 'Stop'

$root = Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')
$sidecar = Join-Path $root 'target\debug\kosmos-local-stt.exe'
if (!(Test-Path $sidecar)) {
  throw "sidecar not found: $sidecar"
}

$psi = [System.Diagnostics.ProcessStartInfo]::new()
$psi.FileName = $sidecar
$psi.UseShellExecute = $false
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$proc = [System.Diagnostics.Process]::Start($psi)
$stderrTask = $proc.StandardError.ReadToEndAsync()

try {
  $payload = [ordered]@{ requestId = 1; op = 'status' } | ConvertTo-Json -Compress
  $bytes = [System.Text.Encoding]::UTF8.GetBytes($payload + "`n")
  $debugHex = [System.BitConverter]::ToString($bytes)
  $proc.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
  $proc.StandardInput.BaseStream.Flush()
  $lineTask = $proc.StandardOutput.ReadLineAsync()
  if (!$lineTask.Wait(5000)) {
    try { $proc.Kill($true) } catch { try { $proc.Kill() } catch {} }
    $stderr = ''
    if ($stderrTask.Wait(5000)) {
      $stderr = $stderrTask.Result
    }
    throw "status timeout after 5000ms; request: $payload; requestHex: $debugHex; stderr: $stderr"
  }
  $line = $lineTask.Result
  if ([string]::IsNullOrWhiteSpace($line)) {
    throw 'empty status response'
  }
  $shutdown = [ordered]@{ requestId = 2; op = 'shutdown' } | ConvertTo-Json -Compress
  $shutdownBytes = [System.Text.Encoding]::UTF8.GetBytes($shutdown + "`n")
  $proc.StandardInput.BaseStream.Write($shutdownBytes, 0, $shutdownBytes.Length)
  $proc.StandardInput.BaseStream.Flush()
  $proc.WaitForExit(5000) | Out-Null
  if (!$proc.HasExited) {
    $proc.Kill($true)
  }
  $stderr = ''
  if ($stderrTask.Wait(5000)) {
    $stderr = $stderrTask.Result
  }
  [pscustomobject]@{
    sidecar = $sidecar
    request = $payload
    requestHex = $debugHex
    response = ($line | ConvertFrom-Json)
    stderr = $stderr
    exitCode = if ($proc.HasExited) { $proc.ExitCode } else { $null }
  } | ConvertTo-Json -Depth 20
} finally {
  if (!$proc.HasExited) {
    try { $proc.Kill($true) } catch {}
  }
  $proc.Dispose()
}
