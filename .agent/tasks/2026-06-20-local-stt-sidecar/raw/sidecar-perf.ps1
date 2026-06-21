$ErrorActionPreference = 'Stop'

$root = Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')
$task = Resolve-Path (Join-Path $root '.agent\tasks\2026-06-20-local-stt-sidecar')
$outJson = Join-Path $task 'raw\sidecar-perf.json'
$outTxt = Join-Path $task 'raw\sidecar-perf.txt'
$timeoutMs = 30000
if ($env:KOSMOS_LOCAL_STT_PERF_TIMEOUT_MS) {
  $timeoutMs = [int]$env:KOSMOS_LOCAL_STT_PERF_TIMEOUT_MS
}

$sidecar = Join-Path $root 'target\debug\kosmos-local-stt.exe'
if (!(Test-Path $sidecar)) {
  $sidecar = Join-Path $root '.tmp\cargo-local-stt-sidecar\debug\kosmos-local-stt.exe'
}
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
$script:startedAt = Get-Date
$proc = [System.Diagnostics.Process]::Start($psi)
$stderrTask = $proc.StandardError.ReadToEndAsync()

$requestId = 0
function Stop-Sidecar {
  if ($script:proc -and !$script:proc.HasExited) {
    Get-CimInstance Win32_Process -Filter "ParentProcessId = $($script:proc.Id)" |
      ForEach-Object {
        try { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue } catch {}
      }
    try {
      $script:proc.Kill($true)
    } catch {
      try { $script:proc.Kill() } catch {}
    }
  }
  Get-Process whisper-server,whisper-cli -ErrorAction SilentlyContinue |
    Where-Object { $_.StartTime -gt $script:startedAt } |
    ForEach-Object {
      try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue } catch {}
    }
}

function Wait-Line($task, $op) {
  if (!$task.Wait($script:timeoutMs)) {
    Stop-Sidecar
    throw "sidecar response timeout after $($script:timeoutMs)ms for $op"
  }
  return $task.Result
}

function Send-Request($payload) {
  $script:requestId += 1
  $payload.requestId = $script:requestId
  $json = $payload | ConvertTo-Json -Compress -Depth 20
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $bytes = [System.Text.Encoding]::UTF8.GetBytes($json + "`n")
  $script:proc.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
  $script:proc.StandardInput.BaseStream.Flush()
  $lineTask = $script:proc.StandardOutput.ReadLineAsync()
  $line = Wait-Line $lineTask $payload.op
  $sw.Stop()
  if ([string]::IsNullOrWhiteSpace($line)) { throw "empty sidecar response for $($payload.op)" }
  [pscustomobject]@{
    op = $payload.op
    elapsedMs = $sw.ElapsedMilliseconds
    request = $payload
    response = ($line | ConvertFrom-Json)
  }
}

try {
  $model = [ordered]@{
    engine = 'whisper.cpp'
    modelId = 'whisper-large-v3-turbo'
    modelPath = $modelPath
    commandPath = $commandPath
    accelerator = 'gpu'
    profile = 'fast'
    idleUnloadAfterMs = 300000
  }
  $wavB64 = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($wav))

  $results = @()
  $results += Send-Request ([ordered]@{ op = 'preload'; model = $model })
  $results += Send-Request ([ordered]@{ op = 'transcribe'; model = $model; wav_base64 = $wavB64; language = 'ru'; prompt = '' })
  $results += Send-Request ([ordered]@{ op = 'status' })
  $results += Send-Request ([ordered]@{ op = 'unload' })
  $results += Send-Request ([ordered]@{ op = 'shutdown' })

  if (!$proc.WaitForExit(5000)) {
    Stop-Sidecar
    $proc.WaitForExit(5000) | Out-Null
  }
  $stderr = ''
  if ($stderrTask.Wait(5000)) {
    $stderr = $stderrTask.Result
  }

  $summary = [ordered]@{
    generatedAt = (Get-Date).ToString('o')
    sidecar = $sidecar
    wav = $wav
    modelPath = $modelPath
    commandPath = $commandPath
    timeoutMs = $timeoutMs
    wavBytes = (Get-Item $wav).Length
    results = $results
    stderr = $stderr
    exitCode = $proc.ExitCode
  }

  $summary | ConvertTo-Json -Depth 30 | Set-Content -Encoding UTF8 $outJson
@"
sidecar: $sidecar
wav: $wav
model: $modelPath
command: $commandPath
timeoutMs: $timeoutMs
preloadMs: $($results[0].elapsedMs)
transcribeMs: $($results[1].elapsedMs)
statusMs: $($results[2].elapsedMs)
unloadMs: $($results[3].elapsedMs)
shutdownMs: $($results[4].elapsedMs)
transcript: $($results[1].response.response.text)
backend: $($results[1].response.response.backend)
statusWarm: $($results[2].response.response.warm)
statusAccelerator: $($results[2].response.response.accelerator)
statusDevice: $($results[2].response.response.device)
statusProfile: $($results[2].response.response.profile)
exitCode: $($proc.ExitCode)
"@ | Set-Content -Encoding UTF8 $outTxt

  Get-Content $outTxt
} finally {
  Stop-Sidecar
  if ($proc) {
    $proc.Dispose()
  }
}
