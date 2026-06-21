$ErrorActionPreference = 'Stop'

$root = Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')
$task = Resolve-Path (Join-Path $root '.agent\tasks\2026-06-20-local-stt-sidecar')
$outJson = Join-Path $task 'raw\sidecar-crash-smoke.json'
$outTxt = Join-Path $task 'raw\sidecar-crash-smoke.txt'
$sidecar = Join-Path $root 'target\debug\kosmos-local-stt.exe'
$wav = Join-Path $env:APPDATA 'Kosmos-dev\dictation\pending\20aef3cd-17b0-4fb0-85be-169450482db0.wav'
$modelPath = Join-Path $env:APPDATA 'Kosmos-dev\models\dictation\ggml-large-v3-turbo.bin'
$commandPath = Join-Path $env:APPDATA 'Kosmos-dev\tools\dictation\whisper.cpp-cublas\Release\whisper-cli.exe'

if (!(Test-Path $sidecar)) { throw "sidecar not found: $sidecar" }
if (!(Test-Path $wav)) { throw "wav not found: $wav" }
if (!(Test-Path $modelPath)) { throw "model not found: $modelPath" }
if (!(Test-Path $commandPath)) { throw "command not found: $commandPath" }

function Start-Sidecar {
  $psi = [System.Diagnostics.ProcessStartInfo]::new()
  $psi.FileName = $script:sidecar
  $psi.UseShellExecute = $false
  $psi.RedirectStandardInput = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.Environment['KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS'] = '25000'
  return [System.Diagnostics.Process]::Start($psi)
}

function Stop-Tree($process, $startedAt) {
  if ($process -and !$process.HasExited) {
    Get-CimInstance Win32_Process -Filter "ParentProcessId = $($process.Id)" |
      ForEach-Object {
        try { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue } catch {}
      }
    try { $process.Kill($true) } catch { try { $process.Kill() } catch {} }
  }
  Get-Process whisper-server,whisper-cli -ErrorAction SilentlyContinue |
    Where-Object { $_.StartTime -gt $startedAt } |
    ForEach-Object {
      try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue } catch {}
    }
}

function Write-JsonLine($process, $payload) {
  $json = $payload | ConvertTo-Json -Compress -Depth 20
  $bytes = [System.Text.Encoding]::UTF8.GetBytes($json + "`n")
  $process.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
  $process.StandardInput.BaseStream.Flush()
}

function Read-LineOrThrow($process, $timeoutMs, $label) {
  $lineTask = $process.StandardOutput.ReadLineAsync()
  if (!$lineTask.Wait($timeoutMs)) {
    throw "sidecar response timeout after ${timeoutMs}ms for $label"
  }
  if ([string]::IsNullOrWhiteSpace($lineTask.Result)) {
    throw "empty sidecar response for $label"
  }
  return $lineTask.Result
}

function New-RepeatedWavBase64($path, $repeat) {
  $source = [System.IO.File]::ReadAllBytes($path)
  if ($source.Length -lt 44) { throw "source wav too short: $path" }
  $dataLen = $source.Length - 44
  $newDataLen = $dataLen * $repeat
  $expanded = New-Object byte[] (44 + $newDataLen)
  [Array]::Copy($source, 0, $expanded, 0, 44)
  [Array]::Copy([BitConverter]::GetBytes([uint32](36 + $newDataLen)), 0, $expanded, 4, 4)
  [Array]::Copy([BitConverter]::GetBytes([uint32]$newDataLen), 0, $expanded, 40, 4)
  for ($i = 0; $i -lt $repeat; $i++) {
    [Array]::Copy($source, 44, $expanded, 44 + ($i * $dataLen), $dataLen)
  }
  return [Convert]::ToBase64String($expanded)
}

$startedAt = Get-Date
$proc = $null
$restart = $null
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
  $beforeExists = Test-Path $wav

  $proc = Start-Sidecar
  Write-JsonLine $proc ([ordered]@{
    requestId = 1
    op = 'transcribe'
    model = $model
    wav_base64 = $wavB64
    language = 'ru'
    prompt = ''
  })

  Start-Sleep -Milliseconds 500
  $killSw = [System.Diagnostics.Stopwatch]::StartNew()
  Stop-Tree $proc $startedAt
  if (!$proc.WaitForExit(5000)) {
    throw "killed sidecar did not exit within 5000ms"
  }
  $killSw.Stop()
  $afterExists = Test-Path $wav

  $restartStartedAt = Get-Date
  $restart = Start-Sidecar
  Write-JsonLine $restart ([ordered]@{ requestId = 1; op = 'status' })
  $statusLine = Read-LineOrThrow $restart 5000 'status after crash restart'
  $status = $statusLine | ConvertFrom-Json
  Write-JsonLine $restart ([ordered]@{ requestId = 2; op = 'shutdown' })
  $shutdownLine = Read-LineOrThrow $restart 5000 'shutdown after crash restart'
  $restart.WaitForExit(5000) | Out-Null
  if (!$restart.HasExited) {
    Stop-Tree $restart $restartStartedAt
  }

  if (!$beforeExists -or !$afterExists) {
    throw "pending WAV existence check failed: before=$beforeExists after=$afterExists"
  }
  if (!$status.ok -or $status.response.kind -ne 'status') {
    throw "fresh sidecar status failed after crash: $statusLine"
  }

  $summary = [ordered]@{
    generatedAt = (Get-Date).ToString('o')
    sidecar = $sidecar
    wav = $wav
    repeatedWavFactor = 80
    killedProcessId = $proc.Id
    killMs = $killSw.ElapsedMilliseconds
    killedExitCode = $proc.ExitCode
    pendingWavExistsBefore = $beforeExists
    pendingWavExistsAfter = $afterExists
    restartProcessId = $restart.Id
    statusAfterRestart = $status
    shutdownAfterRestart = ($shutdownLine | ConvertFrom-Json)
    restartExitCode = if ($restart.HasExited) { $restart.ExitCode } else { $null }
  }

  $summary | ConvertTo-Json -Depth 20 | Set-Content -Encoding UTF8 $outJson
@"
sidecar: $sidecar
wav: $wav
repeatedWavFactor: 80
killMs: $($killSw.ElapsedMilliseconds)
killedExitCode: $($proc.ExitCode)
pendingWavExistsBefore: $beforeExists
pendingWavExistsAfter: $afterExists
statusAfterRestartOk: $($status.ok)
statusAfterRestartKind: $($status.response.kind)
restartExitCode: $(if ($restart.HasExited) { $restart.ExitCode } else { $null })
"@ | Set-Content -Encoding UTF8 $outTxt

  Get-Content $outTxt
} finally {
  if ($proc) {
    Stop-Tree $proc $startedAt
    $proc.Dispose()
  }
  if ($restart) {
    Stop-Tree $restart (Get-Date).AddMinutes(-5)
    $restart.Dispose()
  }
}
