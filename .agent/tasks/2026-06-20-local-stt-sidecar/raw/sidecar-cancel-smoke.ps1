$ErrorActionPreference = 'Stop'

$root = Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')
$sidecar = Join-Path $root 'target\debug\kosmos-local-stt.exe'
$wav = Join-Path $env:APPDATA 'Kosmos-dev\dictation\pending\20aef3cd-17b0-4fb0-85be-169450482db0.wav'
$modelPath = Join-Path $env:APPDATA 'Kosmos-dev\models\dictation\ggml-large-v3-turbo.bin'
$commandPath = Join-Path $env:APPDATA 'Kosmos-dev\tools\dictation\whisper.cpp-cublas\Release\whisper-cli.exe'

if (!(Test-Path $sidecar)) { throw "sidecar not found: $sidecar" }
if (!(Test-Path $wav)) { throw "wav not found: $wav" }
if (!(Test-Path $modelPath)) { throw "model not found: $modelPath" }
if (!(Test-Path $commandPath)) { throw "command not found: $commandPath" }

$psi = [System.Diagnostics.ProcessStartInfo]::new()
$psi.FileName = $sidecar
$psi.UseShellExecute = $false
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.Environment['KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS'] = '25000'
$proc = [System.Diagnostics.Process]::Start($psi)
$stderrTask = $proc.StandardError.ReadToEndAsync()
$startedAt = Get-Date

function Stop-Sidecar {
  if ($script:proc -and !$script:proc.HasExited) {
    Get-CimInstance Win32_Process -Filter "ParentProcessId = $($script:proc.Id)" |
      ForEach-Object {
        try { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue } catch {}
      }
    try { $script:proc.Kill($true) } catch { try { $script:proc.Kill() } catch {} }
  }
  Get-Process whisper-server,whisper-cli -ErrorAction SilentlyContinue |
    Where-Object { $_.StartTime -gt $script:startedAt } |
    ForEach-Object {
      try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue } catch {}
    }
}

function Write-JsonLine($payload) {
  $json = $payload | ConvertTo-Json -Compress -Depth 20
  $bytes = [System.Text.Encoding]::UTF8.GetBytes($json + "`n")
  $script:proc.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
  $script:proc.StandardInput.BaseStream.Flush()
}

function New-RepeatedWavBase64($path, $repeat) {
  $source = [System.IO.File]::ReadAllBytes($path)
  if ($source.Length -lt 44) { throw "source wav too short: $path" }
  $dataLen = $source.Length - 44
  $newDataLen = $dataLen * $repeat
  $wavLen = 44 + $newDataLen
  $expanded = New-Object byte[] $wavLen
  [Array]::Copy($source, 0, $expanded, 0, 44)
  [Array]::Copy([BitConverter]::GetBytes([uint32](36 + $newDataLen)), 0, $expanded, 4, 4)
  [Array]::Copy([BitConverter]::GetBytes([uint32]$newDataLen), 0, $expanded, 40, 4)
  for ($i = 0; $i -lt $repeat; $i++) {
    [Array]::Copy($source, 44, $expanded, 44 + ($i * $dataLen), $dataLen)
  }
  return [Convert]::ToBase64String($expanded)
}

try {
  $model = [ordered]@{
    engine = 'whisper.cpp'
    modelId = 'whisper-large-v3-turbo'
    modelPath = $modelPath
    commandPath = $commandPath
    accelerator = 'gpu'
    profile = 'accurate'
    idleUnloadAfterMs = 300000
  }
  $wavB64 = New-RepeatedWavBase64 $wav 80

  Write-JsonLine ([ordered]@{ requestId = 1; op = 'transcribe'; model = $model; wav_base64 = $wavB64; language = 'ru'; prompt = '' })
  $cancelSw = [System.Diagnostics.Stopwatch]::StartNew()
  Write-JsonLine ([ordered]@{ requestId = 2; op = 'cancel'; target_request_id = 1 })

  $response = $null
  $seen = @()
  while ($cancelSw.ElapsedMilliseconds -lt 5000) {
    $remaining = [Math]::Max(1, 5000 - [int]$cancelSw.ElapsedMilliseconds)
    $lineTask = $proc.StandardOutput.ReadLineAsync()
    if (!$lineTask.Wait($remaining)) {
      break
    }
    $line = $lineTask.Result
    if ([string]::IsNullOrWhiteSpace($line)) {
      continue
    }
    $seen += $line
    $candidate = $line | ConvertFrom-Json
    if ($candidate.requestId -eq 1 -and $candidate.ok -and $candidate.response.kind -eq 'transcription') {
      throw "transcription completed before cancel ack: $line"
    }
    if ($candidate.requestId -eq 2) {
      $response = $candidate
      break
    }
  }
  $cancelSw.Stop()
  if ($null -eq $response) {
    throw "cancel response timeout after 5000ms; seen=$($seen -join ' | ')"
  }
  if (!$response.ok -or $response.response.kind -ne 'ack' -or !$response.response.accepted) {
    throw "cancel was not accepted: $($response | ConvertTo-Json -Compress -Depth 10)"
  }

  Write-JsonLine ([ordered]@{ requestId = 3; op = 'shutdown' })
  $proc.WaitForExit(5000) | Out-Null
  if (!$proc.HasExited) {
    Stop-Sidecar
  }
  $stderr = ''
  if ($stderrTask.Wait(5000)) {
    $stderr = $stderrTask.Result
  }
  [pscustomobject]@{
    sidecar = $sidecar
    wav = $wav
    repeatedWavFactor = 80
    cancelMs = $cancelSw.ElapsedMilliseconds
    response = $response
    seenBeforeAck = $seen
    stderr = $stderr
    exitCode = if ($proc.HasExited) { $proc.ExitCode } else { $null }
  } | ConvertTo-Json -Depth 20
} finally {
  Stop-Sidecar
  if ($proc) {
    $proc.Dispose()
  }
}
