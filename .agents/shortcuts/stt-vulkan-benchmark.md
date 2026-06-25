# STT Vulkan Benchmark

## Trigger

When checking whisper.cpp Vulkan performance on Windows for Kosmos local dictation.

## Symptom

Vulkan is missing from the STT benchmark matrix, `vulkaninfo` is unavailable, or a Vulkan build must prove it is not silently using CPU.

## Do This

Install Vulkan SDK if missing:

```powershell
rtk proxy winget install --id KhronosGroup.VulkanSDK --exact --silent --accept-package-agreements --accept-source-agreements
```

Build whisper.cpp with Vulkan from a Visual Studio Build Tools environment:

```powershell
rtk proxy cmd /c .tmp\build-whisper-vulkan.cmd
```

Verify the binary reports a real device before benchmarking:

```powershell
rtk proxy .tmp\bench\whisper-vulkan-bin-x64\Release\whisper-cli.exe --help
```

Expected proof lines include `ggml_vulkan: Found` and `using Vulkan0 backend` during transcription.

Run only the Vulkan benchmark when isolating it:

```powershell
rtk proxy cmd /c "set KOSMOS_STT_BENCH_BACKENDS=whispercpp-vulkan-dll&& .tmp\run-stt-final-bench.cmd"
```

Run the full STT matrix:

```powershell
rtk proxy cmd /c ".tmp\run-stt-final-bench.cmd > .tmp\stt-final-bench.full.stdout.log 2> .tmp\stt-final-bench.full.stderr.log"
```

## Avoid

Do not trust artifact names or `accelerator: gpu`; require backend logs that say `Vulkan0`. Do not let one sidecar process own the whole matrix without timeouts, because a hung backend can block later CPU/CUDA measurements.

## Promote To Skill When

This becomes a repeated release or CI workflow instead of an ad hoc local benchmark.
