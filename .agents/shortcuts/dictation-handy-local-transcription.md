# Dictation Handy Local Transcription

## Trigger

When investigating Kosmos local dictation speed against the Handy Tauri sample.

## Symptom

Kosmos local transcription is slow, and Handy appears faster because it keeps an in-process `transcribe-rs::WhisperEngine` cached and starts loading the model when recording starts.

## Do This

- Compare against `.tmp/handy-src-tauri/src/actions.rs` and `.tmp/handy-src-tauri/src/managers/transcription.rs` if the sample checkout is present.
- First verify installed Kosmos assets in `%APPDATA%/Kosmos-dev`: model path, `whisper-cli.exe`, and `whisper-server.exe`.
- Smoke both paths with the same WAV/model before changing architecture. On CPU, `ggml-large-v3-turbo.bin` can take about 20s even through `whisper-server.exe`.
- For sidecar perf smoke, use a script with a per-request timeout and `finally` cleanup that kills `kosmos-local-stt.exe`/child processes. A blocking `StandardOutput.ReadLine()` without a deadline can hang the agent for an hour if `preload` never returns.
- Before running desktop dictation Playwright e2e directly, build the shell renderer with `rtk proxy bun run --cwd platform/desktop build:js:shell`. If `platform/desktop/dist/index.html` is missing, Electron launch can time out while logging `ERR_FILE_NOT_FOUND`.
- Match the actual serde wire casing in sidecar JSONL. `LocalSttRequestEnvelope` uses `requestId`, `LocalSttModelSpec` uses camelCase, but `Transcribe` variant fields are snake_case, so the audio field is `wav_base64`, not `wavBase64`.
- If the managed CUDA package has `whisper.dll` next to `whisper-cli.exe`, the sidecar can load it dynamically without adding CMake/libclang to the Rust build. On Windows, use DLL load flags equivalent to `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS` so sibling CUDA/ggml DLLs resolve.
- For cancellation inside native Whisper inference, `JoinHandle::abort()` is not enough once execution is inside blocking C code. Wire `whisper_full_params.abort_callback` to an atomic cancel flag and avoid falling back to `whisper-server.exe` after that flag is set.
- Match Handy's idle behavior with an active watcher, not only lazy request-time checks. Handy checks every 10s; Kosmos sidecar should unload after `idle_unload_after_ms` even if no further `status`/`transcribe` request arrives. Also unload immediately when the user switches dictation provider away from `local`.
- `faster-whisper` cannot consume whisper.cpp `ggml-*.bin` files directly. Do not pass managed `.bin` paths to Python. During `dictation.download_local_model`, create/use the managed venv under `<dataDir>/tools/dictation/faster-whisper/.venv/`, install `faster-whisper`, prepare a separate CTranslate2/HF cache, then store the backend model id such as `large-v3-turbo` as `localModelPath`; use `KOSMOS_FASTER_WHISPER_PYTHON` / `KOSMOS_FASTER_WHISPER_MODEL` only as explicit diagnostic overrides.
- For faster-whisper on CUDA, add the managed whisper.cpp CUDA `Release` directory to the Python child `PATH`; CTranslate2 needs DLLs such as `cublas64_12.dll`/`cublasLt64_12.dll`. If missing, a manual smoke can load the model in ~2s and then fail during encode with `Library cublas64_12.dll is not found or cannot be loaded`.
- On Windows, do not let the faster-whisper Python child inherit the sidecar stdin pipe. Set `stdin(Stdio::null())`; otherwise `python.exe` can be created but never reach the first script line, and the sidecar returns `faster-whisper timed out after 55s`.
- For faster-whisper CUDA DLL loading, `PATH` alone can be insufficient or point at the CPU `Release` dir. Pass all known runtime DLL dirs, especially `<dataDir>/tools/dictation/whisper.cpp-cublas/Release`, and call `os.add_dll_directory(...)` in the Python script before importing/running CTranslate2.
- Honor `KOSMOS_FASTER_WHISPER_DLL_DIRS` / `KOSMOS_FASTER_WHISPER_DLL_DIR` as first-class inputs when building the Python child DLL path. Diagnostics often run outside the desktop dev data-dir bootstrap; ignoring these env vars makes CUDA smoke tests fail with `cublas64_12.dll is not found` even when the managed CUDA release exists.
- Faster Whisper must be a persistent sidecar worker, not one Python process per utterance. On RTX 5070 with `large-v3-turbo`, measured warm transcription of a short spoken WAV was 703ms then 399ms after a 3.7s preload when the model was already cached. A previous cold run loaded/warmed in ~27s, then the same pending WAV transcribed in ~0.3s. If it is slower than warm whisper.cpp, verify `Preload` really starts/reuses a worker and does a warm-up inference.
- Do not start faster-whisper prewarm only when recording begins. `dictation.start_recording` returns while preload continues in the background, so a short recording can race the warm-up and make `submit_audio` wait several seconds. Prewarm on backend startup and after switching config to a local provider/engine.
- Faster-whisper cold preload is several seconds even on a high-end NVIDIA GPU, while warm inference is sub-second. Do not use the 5-minute Handy-style idle unload as the default for faster-whisper; keep its worker hot unless `KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS` explicitly overrides it. Still unload immediately when switching away from the local provider.
- Faster-whisper worker reuse must compare an effective runtime key, not the full `LocalSttModelSpec`. In the live app, preload can send `modelId=null`/`idleUnloadAfterMs=null`, while transcribe sends the selected `modelId`/config metadata for the same `modelPath`; strict struct equality reloads the Python worker and turns every first transcribe after preload into another 4-6s cold load.
- The dictation pill success path can still call `dictation.cancel` after `submit_audio` succeeds. Do not let an idle/success cancel reset the local sidecar warm state; otherwise every successful dictation kills `FasterWhisperWorker` and the next utterance pays cold preload again. Only cancel/reset sidecar when an active recording/transcription is actually being aborted.
- For short faster-whisper dictation clips, VAD can cost more than it saves and can turn very short recordings into `EmptyTranscript`. Keep `condition_on_previous_text=False`, but gate `vad_filter` by audio duration and do not drop the warm worker on `EmptyTranscript`.
- Verify actual GPU use during preload/transcribe with `nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv,noheader`. In a working run, status reported `device="NVIDIA GeForce RTX 5070"` and `nvidia-smi` showed the faster-whisper Python process while preload was active.
- For dictation hallucination control in faster-whisper, keep warm-up `vad_filter=False` on the synthetic silence WAV, but use `condition_on_previous_text=False`, `vad_filter=True`, and a short `vad_parameters.min_silence_duration_ms` for real transcribe requests.
- Keep `dictation.submit_audio` timeouts short for desktop UX. A useful split is 60s for Electron/ARK IPC and a lower internal faster-whisper child timeout, so a stuck Python backend returns a local STT error before the transport times out.
- If using the existing external binary path, preload/reuse `whisper-server.exe` at recording start and submit audio to `/inference`.
- On NVIDIA machines, the managed CPU `whisper-bin-x64.zip` path is the main speed trap. Prefer a CUDA `whisper.cpp` package and keep the server warm. In one RTX 5070 check, CUDA server startup was about 3s and warm `/inference` on a tiny WAV was about 370ms.
- If GitHub release asset downloads reset before the first byte, try the SourceForge whisper.cpp mirror for the same CUDA zip.
- For the pill overlay, `transparent: true` plus CSS transparency may still show a white native gutter on Windows. Use `BrowserWindow.setShape(...)` and keep only a tiny native margin around the visible pill. A zero-margin shape can clip the pill's antialiased border; in the current implementation the window is `124x40`, the visible pill is `120x36`, and the pill sits at `(2,2)`.

## Avoid

- Do not add mandatory `transcribe-rs` directly unless the Windows build toolchain is available. The `whisper-cpp` feature needs CMake; the Vulkan path needs libclang/Vulkan SDK or generated bindings support.
- Before attempting embedded `whisper-rs`/`transcribe-rs`/`whispercpp`, check `cmake --version`, `clang --version`, and `cl`. In the 2026-06-21 sidecar proof-loop, none were on PATH, while the managed CUDA package had `whisper.dll` but no `.lib` import library.
- Do not treat server startup as the full latency problem; model inference can dominate after the process is already warm.
- Do not run real-whisper perf scripts from the agent loop unless both the outer tool call and the script itself have hard timeouts.
- Do not “fix” faster-whisper hangs by only raising IPC timeout. First verify Python package availability, model format, and whether the child process has its own deadline.
- Do not run broad e2e without a fresh renderer build, and do not leave timed-out `bunx`/`electron`/`node` children running. Kill only processes started after the failed run's timestamp.
- Do not assume every nested protocol field is camelCase just because the envelope is camelCase; check `local_sidecar_protocol.rs` or use Rust serialization output.
- Do not use PowerShell `$vars` inside a double-quoted `rtk proxy powershell -Command` payload; the outer shell can expand them away. Use Node for timing scripts or avoid `$` in the inner command.
- Do not only shrink the overlay window if the bug is a native white background; that hides most of the symptom but leaves white corner/edge pixels. Do not make the native shape exactly the same size as the CSS pill either, because Windows can visibly cut the rounded edges.

## Promote To Skill When

This becomes a recurring area of dictation work beyond local speed debugging.
