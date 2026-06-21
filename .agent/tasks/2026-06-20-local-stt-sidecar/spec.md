# 2026-06-20 local-stt-sidecar

## Context

Kosmos local dictation currently uses downloaded whisper.cpp binaries through
`whisper-server.exe` when available, with `whisper-cli.exe` as a fallback. On
the current Windows dev machine the selected runtime is the CUDA cublas build,
but the architecture still depends on a generic external HTTP server lifecycle.

The target architecture is a Kosmos-owned local STT sidecar process:
`kosmos-local-stt.exe`. It should embed the transcription engine in that
sidecar process, keep the main runtime isolated from native whisper crashes,
and expose a narrow IPC protocol for preload, transcription, cancellation,
status, and unload.

## Scope

In this task:

- Design and implement `kosmos-local-stt.exe` as a separate process owned by
  Kosmos, not as in-process whisper inside the main runtime.
- Implement an IPC contract between `platform/runtime` and the sidecar for:
  `status`, `load_model`, `preload`, `transcribe`, `cancel`, `unload`.
- Keep existing local dictation behavior intact from the UI perspective:
  Settings selects a local model, recording produces a transcript, pending
  audio remains disk-first, and injection behavior does not change.
- Add model lifecycle management: eager preload, warm/cold status, idle unload,
  and explicit unload.
- Add accelerator visibility and control for at least `auto`, `cpu`, and `gpu`
  modes, with selected GPU device reported when available.
- Preserve a debug escape hatch for raw `whisper-cli.exe`, but remove it from
  the normal product path.
- Add perf instrumentation for cold start, warm transcription, model load time,
  and unload time.
- Add regression tests around routing, fallback, lifecycle, and failure
  recovery.
- Update `docs-site/concepts/dictation.md` and generated docs if the
  architecture changes.

Not in this task:

- Cloud Groq transcription changes.
- Audio capture redesign or streaming AudioWorklet protocol.
- Changing dictation hotkey/PTT behavior.
- Changing pending queue semantics or retention policy.
- Replacing the model catalog itself, except metadata needed for sidecar
  runtime/accelerator selection.
- ARK schema or sync changes.

## Architecture Decision

The desired final shape is:

- `platform/runtime` owns dictation state, pending queue, retries, config, and
  injection.
- `kosmos-local-stt.exe` owns native STT engine loading, GPU/backend selection,
  in-memory model lifecycle, transcription execution, cancellation, and unload.
- IPC is local-only and typed. Preferred transport is stdio JSON-RPC or named
  pipe JSON-RPC. The implementation must justify the chosen transport in
  `evidence.md`.
- The main runtime must treat sidecar crashes as recoverable local STT failures:
  mark the attempt failed/queued according to existing retry semantics, then
  restart the sidecar on the next preload/transcribe request.

## Discovery Gates

Before implementation:

- DG1. Benchmark current `whisper-server.exe` cold preload, warm inference, and
  CLI fallback on the same local WAV/model.
- DG2. Confirm the selected current binary path, model path, accelerator mode,
  and GPU device through config and runtime output.
- DG3. Compare candidate sidecar engine options:
  `transcribe-rs`, direct `whisper-rs`, or a thin wrapper around whisper.cpp C
  API. Record build requirements and Windows packaging risk.
- DG4. Choose IPC transport and document why it is enough for cancellation,
  timeout, crash recovery, logs, and future streaming.

Discovery outputs go in:

- `.agent/tasks/2026-06-20-local-stt-sidecar/evidence.md`
- `.agent/tasks/2026-06-20-local-stt-sidecar/raw/`

## Acceptance Criteria

AC1. The normal local dictation product path uses `kosmos-local-stt.exe`, not
`whisper-server.exe` or `whisper-cli.exe`, when a managed local model is
selected and sidecar startup succeeds.

AC2. The main Kosmos runtime never links the native whisper engine directly.
Native STT engine code lives in the sidecar crate/binary boundary, so a sidecar
process crash does not crash `kepler-backend`.

AC3. Sidecar IPC supports `status`, `load_model`, `preload`, `transcribe`,
`cancel`, and `unload`, with typed request/response structs and tests for
serialization compatibility.

AC4. Recording start preloads the selected local model before audio submission
when provider is `local`; submitting audio reuses the warm sidecar model if the
same model/accelerator/device is still loaded.

AC5. Idle unload works: after the configured idle timeout, the sidecar releases
the loaded model and reports `warm: false` / `loadedModel: null` without
terminating the main runtime.

AC6. Sidecar crash recovery works: if the sidecar exits during preload or
transcription, the current attempt returns a controlled local STT error, the
pending WAV is not deleted, and the next local transcription can start a fresh
sidecar.

AC7. Cancellation works: cancelling dictation while local STT is loading or
transcribing stops the current sidecar request and returns the host state to
idle/error according to existing dictation state semantics without injecting
partial text.

AC8. Accelerator mode is observable: Settings or dictation state exposes the
effective local STT accelerator (`auto`, `cpu`, `gpu`) and selected device when
available. On the current NVIDIA dev machine, `gpu` mode must report the RTX
device or a concrete failure reason.

AC9. Quality/speed profiles are configurable internally as at least `fast` and
`accurate`. `fast` preserves greedy-style behavior; `accurate` enables a
higher-quality beam-search profile when supported by the engine/backend.

AC10. Perf evidence exists for the current machine and model:
current `whisper-server.exe` baseline vs sidecar cold preload vs sidecar warm
transcription. Evidence must include model id/path, binary path, accelerator,
WAV duration, and measured timings.

AC11. Existing dictation pending queue guarantees remain intact: WAV files are
written before transcription, failed local STT attempts keep pending files, and
successful transcript+inject cleanup still removes completed pending items.

AC12. Documentation is updated from source docs and regenerated:
`docs-site/concepts/dictation.md` describes the sidecar architecture, and
`bun run docs:sync` plus `bun run docs:check` pass.

## Verification Commands

- `cargo test --manifest-path platform/runtime/Cargo.toml dictation` verifies
  host routing, pending behavior, lifecycle, and crash handling tests.
- `cargo test --manifest-path platform/runtime/Cargo.toml local_stt` verifies
  sidecar protocol and local STT adapter tests, if tests are named separately.
- `bun run ark:guard:writes` verifies no forbidden ARK write paths were added.
- `bun run --cwd platform/desktop test` or the closest existing desktop unit
  suite verifies Settings/dictation UI contract changes.
- `KOSMOS_HEADLESS=1 bunx playwright test --config platform/desktop/playwright.config.ts platform/desktop/e2e/dictation.spec.ts`
  verifies the headless dictation flow.
- `bun run docs:sync` regenerates root docs after source doc changes.
- `bun run docs:check` verifies docs consistency.
- Manual perf smoke on the current Windows/NVIDIA machine records cold/warm
  timings and sidecar status output in `evidence.md`.

## Evidence Requirements

`evidence.md` must include:

- A table mapping every AC to `PASS`, `FAIL`, or `UNKNOWN`.
- Exact commands run and their summarized output.
- Links to raw logs under `raw/` for noisy commands.
- Current-machine perf table with:
  model, WAV duration, accelerator, cold preload ms, warm transcribe ms, total
  stop-to-transcript ms, and unload ms.
- Crash-recovery evidence showing sidecar kill during transcription does not
  crash the main runtime and does not delete the pending WAV.

`evidence.json` must include the same AC verdicts in machine-readable form.

## Problems Policy

If any AC is not `PASS`, create `problems.md` with:

- The failed AC.
- What was expected.
- What actually happened.
- Minimal proposed fix.
- Reverify command.

Do not mark the task complete while any AC is `FAIL` or `UNKNOWN`.

## Out Of Scope Decisions

- Realtime streaming STT remains out of scope. This sidecar can choose IPC that
  does not block future streaming, but it does not need to implement streaming
  audio chunks in this task.
- Cloud provider routing remains unchanged.
- UI redesign is out of scope except minimal Settings/status surface needed to
  expose accelerator and warm/cold state.
