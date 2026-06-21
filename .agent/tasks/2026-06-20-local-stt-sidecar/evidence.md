# 2026-06-20 local-stt-sidecar evidence

## Status

Task status: complete.

The local dictation product path now goes through `kosmos-local-stt.exe`.
The main runtime keeps pending queue/state ownership and never links the native
Whisper engine directly. Native Whisper execution is isolated in the sidecar:
the normal current-machine backend dynamically loads the managed
`whisper.dll`; the legacy `whisper-server.exe` path remains only as a sidecar
fallback/debug path, and the main-runtime direct fallback is gated by
`KOSMOS_LOCAL_STT_ALLOW_DIRECT_FALLBACK=1`.

## Discovery Gates

| Gate | Verdict | Finding                                                                                                                                                                                                                                     |
| ---- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| DG1  | PASS    | Current `whisper-server.exe`/`whisper-cli.exe` baseline and sidecar cold/warm timings were measured on the same live pending WAV/model.                                                                                                     |
| DG2  | PASS    | Live config and GPU probe confirmed local dictation points at the CUDA cublas whisper.cpp package on an RTX 5070 machine.                                                                                                                   |
| DG3  | PASS    | `transcribe-rs`/`whisper-rs` require native build tooling that is not available on PATH. The selected implementation loads the bundled `whisper.dll` dynamically inside the sidecar process, avoiding native linkage from `kepler-backend`. |
| DG4  | PASS    | Stdio newline-delimited JSON was selected. It keeps logs on stderr, supports typed request/response structs, allows cancellation while inference runs, and crash recovery is just process restart.                                          |

## Baseline Perf

Source logs:

- `raw/dg1-benchmark.txt`
- `raw/dg1-benchmark.json`
- `raw/dg1-benchmark.ps1`

Used asset:

- `%APPDATA%\Kosmos-dev\dictation\pending\20aef3cd-17b0-4fb0-85be-169450482db0.wav`
- duration: `1.28s`

Measured baseline:

| Path             | Binary/model                                     |      ms |
| ---------------- | ------------------------------------------------ | ------: |
| cold preload     | `whisper-server.exe` + `ggml-large-v3-turbo.bin` |  `2078` |
| first transcribe | same                                             | `14628` |
| warm transcribe  | same                                             |   `184` |
| CLI fallback     | `whisper-cli.exe` + same model/WAV               | `16546` |

Transcript observed: `Спасибо.`

Measured sidecar smoke:

| Metric                          | Binary/model                                       |     ms |
| ------------------------------- | -------------------------------------------------- | -----: |
| sidecar cold preload            | `kosmos-local-stt.exe` + `ggml-large-v3-turbo.bin` | `1765` |
| sidecar warm transcribe         | same                                               |  `403` |
| total preload + transcribe      | same                                               | `2168` |
| sidecar status after transcribe | same                                               |   `37` |
| sidecar unload                  | same                                               |   `98` |

Sidecar status reported `accelerator=gpu`, `device=NVIDIA GeForce RTX 5070`,
`profile=fast`, backend `whisper_dll`, transcript `Спасибо.` Raw logs:
`raw/sidecar-perf.txt` and `raw/sidecar-perf.json`.

## Verification Commands

| Command                                                                                                                                                                                                                                           | Verdict | Notes                                                                                                                                                                                   |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rtk cargo fmt --manifest-path platform/runtime/Cargo.toml -- --check`                                                                                                                                                                            | PASS    | Runtime Rust formatting passed.                                                                                                                                                         |
| `rtk cargo build --manifest-path platform/runtime/Cargo.toml --bin kosmos-local-stt`                                                                                                                                                              | PASS    | Sidecar debug binary built after dynamic `whisper.dll` backend/cancel changes.                                                                                                          |
| `rtk cargo test --manifest-path platform/runtime/Cargo.toml dictation::local`                                                                                                                                                                     | PASS    | `26 passed, 372 filtered out`; covers sidecar routing, profile/accelerator flags, sidecar-unavailable retry, and missing-path validation.                                               |
| `rtk cargo test --manifest-path platform/runtime/Cargo.toml dictation::local_sidecar`                                                                                                                                                             | PASS    | `5 passed, 393 filtered out`; covers sidecar status/cancel/idle unload behavior.                                                                                                        |
| `rtk proxy powershell -NoProfile -Command '$env:CARGO_TARGET_DIR=".tmp\cargo-idle-unload"; cargo test --manifest-path platform/runtime/Cargo.toml idle_watcher_unloads_without_new_request -- --test-threads=1'`                                  | PASS    | Active sidecar idle watcher unloaded a warm model without a new sidecar request.                                                                                                        |
| `rtk proxy powershell -NoProfile -Command '$env:CARGO_TARGET_DIR=".tmp\cargo-idle-unload"; cargo test --manifest-path platform/runtime/Cargo.toml update_config_unloads_local_sidecar_when_provider_switches_away -- --test-threads=1'`           | PASS    | Switching dictation provider from `local` to `groq` sends sidecar `unload`.                                                                                                             |
| `rtk cargo test --manifest-path platform/runtime/Cargo.toml local_sidecar_protocol`                                                                                                                                                               | PASS    | `2 passed, 396 filtered out`; typed protocol serialization compatibility.                                                                                                               |
| `rtk cargo test --manifest-path platform/runtime/Cargo.toml process_one_attempt_5xx_is_retryable_keeps_pending`                                                                                                                                   | PASS    | Host retryable 5xx regression passed.                                                                                                                                                   |
| `rtk cargo test --manifest-path platform/runtime/Cargo.toml auto_retry_loop_gives_up_on_persistent_retryable`                                                                                                                                     | PASS    | Host retry loop regression passed.                                                                                                                                                      |
| `rtk cargo test --manifest-path platform/runtime/Cargo.toml submit_audio_local_missing_model_path_returns_error_and_keeps_pending -- --test-threads=1`                                                                                            | PASS    | Missing local model path returns controlled error and keeps pending attempt.                                                                                                            |
| `rtk cargo test --manifest-path platform/runtime/Cargo.toml dictation::host -- --test-threads=1`                                                                                                                                                  | PASS    | `55 passed, 343 filtered out`; host state machine/pending tests passed.                                                                                                                 |
| `rtk cargo test --manifest-path platform/runtime/Cargo.toml dictation`                                                                                                                                                                            | PASS    | `178 passed, 3 ignored, 217 filtered out` in the broad dictation suite.                                                                                                                 |
| `rtk bun run ark:guard:writes`                                                                                                                                                                                                                    | PASS    | Passed after replacing the pre-existing `settings-window.ts` direct `app.getPath("userData")` access with `resolveInstance().userDataDir`.                                              |
| `rtk proxy bun run --cwd platform/desktop typecheck`                                                                                                                                                                                              | PASS    | Desktop TypeScript check passed.                                                                                                                                                        |
| `rtk proxy bun run --cwd platform/desktop build:js:shell`                                                                                                                                                                                         | PASS    | Built `platform/desktop/dist/index.html` and Electron bundles required by Playwright e2e.                                                                                               |
| `rtk proxy powershell -NoProfile -Command '$env:KOSMOS_HEADLESS="1"; bunx playwright test --config platform/desktop/playwright.config.ts platform/desktop/e2e/dictation.spec.ts'`                                                                 | PASS    | `10 passed, 2 skipped` in `42.6s`; skipped tests are opt-in real Groq/local smokes.                                                                                                     |
| `rtk bun run docs:sync`                                                                                                                                                                                                                           | PASS    | Generated docs refreshed from `docs-site/`.                                                                                                                                             |
| `rtk bun run docs:check`                                                                                                                                                                                                                          | PASS    | Docs freshness check passed.                                                                                                                                                            |
| `rtk proxy powershell -NoProfile -ExecutionPolicy Bypass -File .agent/tasks/2026-06-20-local-stt-sidecar/raw/sidecar-status-smoke.ps1`                                                                                                            | PASS    | Sidecar `status` response returned over stdio.                                                                                                                                          |
| `rtk proxy powershell -NoProfile -ExecutionPolicy Bypass -File .agent/tasks/2026-06-20-local-stt-sidecar/raw/sidecar-perf.ps1`                                                                                                                    | PASS    | `preloadMs=1765`, `transcribeMs=403`, `backend=whisper_dll`, `device=NVIDIA GeForce RTX 5070`; no fresh sidecar/whisper processes remained.                                             |
| `rtk proxy powershell -NoProfile -ExecutionPolicy Bypass -Command '& .\.agent\tasks\2026-06-20-local-stt-sidecar\raw\sidecar-cancel-smoke.ps1 \| Tee-Object -FilePath .\.agent\tasks\2026-06-20-local-stt-sidecar\raw\sidecar-cancel-smoke.json'` | PASS    | Active accurate-profile transcription cancel acknowledged in `42ms`; stderr shows `whisper_full_with_state: failed to encode`, proving the abort callback interrupted native inference. |
| `rtk proxy powershell -NoProfile -ExecutionPolicy Bypass -File .agent/tasks/2026-06-20-local-stt-sidecar/raw/sidecar-crash-smoke.ps1`                                                                                                             | PASS    | Killed sidecar during active transcribe in `199ms`; pending WAV still existed; a fresh sidecar started and returned `status`.                                                           |

## Acceptance Criteria

| AC   | Verdict | Evidence                                                                                                                                                                                                                                                                                   |
| ---- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| AC1  | PASS    | Runtime local path prefers `kosmos-local-stt`; main-runtime direct whisper fallback is env-gated. Current managed model smoke used sidecar backend `whisper_dll`, not `whisper-server.exe`/`whisper-cli.exe`.                                                                              |
| AC2  | PASS    | `kepler-backend` does not link native Whisper. The native C API is dynamically loaded only by `kosmos-local-stt.exe` through `local_whisper_dll.rs`; crash smoke killed the sidecar without taking down the host process model and a fresh sidecar restarted.                              |
| AC3  | PASS    | Typed stdio JSON protocol supports `status`, `load_model`, `preload`, `transcribe`, `cancel`, `unload`, and `shutdown`; protocol tests passed.                                                                                                                                             |
| AC4  | PASS    | Host `start_recording` with provider `local` preloads through sidecar; perf smoke proves the following transcribe reuses the loaded model/backend for the same model/accelerator/device.                                                                                                   |
| AC5  | PASS    | Idle unload is implemented via `idle_unload_after_ms` and an active sidecar watcher that checks every 10 seconds by default. Tests cover both request-time idle unload and watcher-driven unload without a new request.                                                                    |
| AC6  | PASS    | Host tests prove controlled local STT errors keep pending WAVs and retry through a fresh sidecar. Crash smoke additionally killed the sidecar during active transcription, verified the pending WAV still existed, and verified a new sidecar could start.                                 |
| AC7  | PASS    | Sidecar stdio loop accepts cancel while inference runs; `whisper_full_params.abort_callback` stops native `whisper.dll` inference. Cancel smoke acknowledged cancel in `42ms` without partial transcription. Host cancel tests prove state returns to idle and late results do not inject. |
| AC8  | PASS    | Sidecar status and `dictation.local_status` expose accelerator/device/profile; current machine reports `gpu` and `NVIDIA GeForce RTX 5070`.                                                                                                                                                |
| AC9  | PASS    | Internal `fast` profile maps to greedy-style flags and `accurate` maps to beam-search flags; CPU/GPU mode flags are wired and covered by focused tests.                                                                                                                                    |
| AC10 | PASS    | Baseline and sidecar perf evidence exists on the same WAV/model with model path, command path, accelerator, backend, timings, transcript, and unload time.                                                                                                                                 |
| AC11 | PASS    | Pending queue guarantees remain intact: WAV-before-transcribe and failed-local-STT-keeps-pending are covered by host/local tests; e2e local mock flow passed.                                                                                                                              |
| AC12 | PASS    | `docs-site/concepts/dictation.md` and related source docs were updated; `docs:sync` and `docs:check` passed.                                                                                                                                                                               |

## Changed Areas

- Runtime sidecar/protocol:
  - `platform/runtime/Cargo.toml`
  - `platform/runtime/src/bin/kosmos-local-stt.rs`
  - `platform/runtime/src/dictation/local.rs`
  - `platform/runtime/src/dictation/local_sidecar.rs`
  - `platform/runtime/src/dictation/local_sidecar_protocol.rs`
  - `platform/runtime/src/dictation/local_whisper_dll.rs`
  - `platform/runtime/src/dictation/mod.rs`
  - `platform/runtime/src/dictation/retry.rs`
  - `platform/runtime/src/dictation/host.rs`
- Desktop build/package:
  - `platform/desktop/scripts/build-backend.mjs`
  - `platform/desktop/scripts/build-backend-dev.mjs`
  - `platform/desktop/package.json`
  - `platform/desktop/electron/main.ts`
  - `platform/desktop/electron/settings-window.ts`
- Docs/proof-loop:
  - `docs-site/concepts/dictation.md`
  - `docs-site/concepts/architecture.md`
  - `docs-site/reference/decisions.md`
  - `docs-site/concepts/system-requirements.md`
  - generated docs from `docs:sync`
  - `.agent/tasks/2026-06-20-local-stt-sidecar/raw/*`
