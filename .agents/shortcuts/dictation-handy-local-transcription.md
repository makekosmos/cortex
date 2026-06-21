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
- `faster-whisper` cannot consume whisper.cpp `ggml-*.bin` files directly. Do not pass managed `.bin` paths to Python. Prepare a separate faster-whisper/CTranslate2 cache during `dictation.download_local_model`, then store the backend model id such as `large-v3-turbo` as `localModelPath`; use `KOSMOS_FASTER_WHISPER_MODEL` only as an explicit diagnostic override.
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
