$ErrorActionPreference = 'Stop'

$root = 'C:\Users\kirill\Coding\kosmos'
$task = Join-Path $root '.agent\tasks\2026-06-20-local-stt-sidecar'
$raw = Join-Path $task 'raw'
New-Item -ItemType Directory -Force -Path $raw | Out-Null

$wav = 'C:\Users\kirill\AppData\Roaming\Kosmos-dev\dictation\pending\20aef3cd-17b0-4fb0-85be-169450482db0.wav'
$model = 'C:\Users\kirill\AppData\Roaming\Kosmos-dev\models\dictation\ggml-large-v3-turbo.bin'
$server = 'C:\Users\kirill\AppData\Roaming\Kosmos-dev\tools\dictation\whisper.cpp-cublas\Release\whisper-server.exe'
$cli = 'C:\Users\kirill\AppData\Roaming\Kosmos-dev\tools\dictation\whisper.cpp-cublas\Release\whisper-cli.exe'
$serverStdout = Join-Path $raw 'dg1-server.stdout.log'
$serverStderr = Join-Path $raw 'dg1-server.stderr.log'
$cliOutBase = Join-Path $raw 'dg1-cli-output'

Remove-Item $serverStdout -ErrorAction SilentlyContinue
Remove-Item $serverStderr -ErrorAction SilentlyContinue
Remove-Item "$cliOutBase.txt" -ErrorAction SilentlyContinue

$threads = [Math]::Min([Environment]::ProcessorCount, 8)

$wavBytes = [IO.File]::ReadAllBytes($wav)
$channels = [BitConverter]::ToUInt16($wavBytes, 22)
$rate = [BitConverter]::ToUInt32($wavBytes, 24)
$bits = [BitConverter]::ToUInt16($wavBytes, 34)
$duration = [Math]::Round((($wavBytes.Length - 44) / ($rate * $channels * ($bits / 8.0))), 3)

$portListener = [System.Net.Sockets.TcpListener]::Create(0)
$portListener.Start()
$port = ([System.Net.IPEndPoint]$portListener.LocalEndpoint).Port
$portListener.Stop()

$serverArgs = @(
  '--host', '127.0.0.1',
  '--port', $port.ToString(),
  '--inference-path', '/inference',
  '-m', $model,
  '-t', $threads.ToString(),
  '-nt',
  '-bo', '1',
  '-bs', '1'
)

$serverProc = Start-Process -FilePath $server -ArgumentList $serverArgs -PassThru -WindowStyle Hidden `
  -RedirectStandardOutput $serverStdout -RedirectStandardError $serverStderr

try {
  $coldStart = [System.Diagnostics.Stopwatch]::StartNew()
  $ready = $false
  for ($i = 0; $i -lt 240; $i++) {
    try {
      & curl.exe --silent --show-error --fail "http://127.0.0.1:$port/" | Out-Null
      $ready = $true
      break
    } catch {
      Start-Sleep -Milliseconds 250
    }
  }
  if (-not $ready) {
    throw "whisper-server did not become ready on port $port"
  }
  $coldStart.Stop()

  $formArgs = @(
    '-F', "file=@$wav;type=audio/wav",
    '-F', 'response_format=json',
    '-F', 'language=ru',
    '-F', 'temperature=0.0',
    '-F', 'temperature_inc=0.2',
    '-F', 'no_speech_thold=0.6',
    "http://127.0.0.1:$port/inference"
  )

  $firstSw = [System.Diagnostics.Stopwatch]::StartNew()
  $first = & curl.exe --silent --show-error --fail @formArgs 2>&1
  $firstSw.Stop()

  $secondSw = [System.Diagnostics.Stopwatch]::StartNew()
  $second = & curl.exe --silent --show-error --fail @formArgs 2>&1
  $secondSw.Stop()

  $cliArgs = @(
    '-m', $model,
    '-f', $wav,
    '-otxt',
    '-of', $cliOutBase,
    '-nt',
    '-np',
    '-t', $threads.ToString(),
    '-bo', '1',
    '-bs', '1',
    '-l', 'ru'
  )

  $cliSw = [System.Diagnostics.Stopwatch]::StartNew()
  $cliOutput = & $cli @cliArgs 2>&1
  $cliSw.Stop()

  $cliTranscript = if (Test-Path "$cliOutBase.txt") {
    (Get-Content "$cliOutBase.txt" -Raw).Trim()
  } else {
    ''
  }

  $summary = [ordered]@{
    kind = 'dg1-benchmark'
    timestamp = (Get-Date).ToString('o')
    model = $model
    wav = $wav
    durationSec = $duration
    accelerator = 'gpu'
    threads = $threads
    commandPaths = [ordered]@{
      whisperServer = $server
      whisperCli = $cli
    }
    whisperServer = [ordered]@{
      coldPreloadMs = $coldStart.ElapsedMilliseconds
      firstTranscribeMs = $firstSw.ElapsedMilliseconds
      warmTranscribeMs = $secondSw.ElapsedMilliseconds
      firstTranscript = ($first | Select-Object -First 1)
      secondTranscript = ($second | Select-Object -First 1)
    }
    whisperCli = [ordered]@{
      fallbackMs = $cliSw.ElapsedMilliseconds
      transcript = $cliTranscript
      rawOutput = ($cliOutput | Select-Object -First 1)
    }
    port = $port
  }

  $jsonPath = Join-Path $raw 'dg1-benchmark.json'
  $txtPath = Join-Path $raw 'dg1-benchmark.txt'
  $summary | ConvertTo-Json -Depth 6 | Set-Content -Path $jsonPath -Encoding UTF8

  @(
    'DG1 benchmark'
    "model: $model"
    "wav: $wav"
    "durationSec: $duration"
    "accelerator: gpu"
    "threads: $threads"
    "server: $server"
    "cli: $cli"
    "coldPreloadMs: $($coldStart.ElapsedMilliseconds)"
    "firstTranscribeMs: $($firstSw.ElapsedMilliseconds)"
    "warmTranscribeMs: $($secondSw.ElapsedMilliseconds)"
    "cliFallbackMs: $($cliSw.ElapsedMilliseconds)"
    "firstTranscript: $($first | Select-Object -First 1)"
    "secondTranscript: $($second | Select-Object -First 1)"
    "cliTranscript: $cliTranscript"
  ) | Set-Content -Path $txtPath -Encoding UTF8

  $summary
}
finally {
  if ($serverProc -and -not $serverProc.HasExited) {
    Stop-Process -Id $serverProc.Id -Force
  }
}
