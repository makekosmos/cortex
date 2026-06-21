# 2026-06-21 faster-whisper-backend evidence

## Status

Task status: complete for the selectable backend slice plus the first
faster-whisper timeout follow-up.

`whisper.cpp` remains the default. `faster-whisper` is available as an optional
local engine value and is routed through the same `kosmos-local-stt` sidecar
boundary. The current implementation expects an existing Python environment
with the `faster_whisper` package. Managed whisper.cpp `.bin` selections are
mapped to equivalent faster-whisper model ids where possible; custom paths must
be compatible CTranslate2 model directories. Automatic conversion/download is
intentionally out of scope for this slice.

## Acceptance Criteria

| AC  | Verdict | Evidence                                                                                                                                                                                                                                                                           |
| --- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1 | PASS    | Existing default config still uses `localEngine = "whisper.cpp"`.                                                                                                                                                                                                                  |
| AC2 | PASS    | Settings exposes `Faster Whisper`; existing `localEngine` persistence path stores the selected value and host no longer resets it when selecting a downloaded model.                                                                                                               |
| AC3 | PASS    | Sidecar protocol carries `engine`; `request_roundtrip_preserves_faster_whisper_engine` covers the `faster-whisper` value.                                                                                                                                                          |
| AC4 | PASS    | Sidecar `preload` marks faster-whisper warm without loading whisper.cpp; `transcribe` routes faster-whisper to the Python runner and whisper.cpp to the existing DLL/server path. Managed ggml `.bin` models map to faster-whisper model ids.                                      |
| AC5 | PASS    | Missing faster-whisper Python runtime returns a controlled `faster-whisper failed` / launch error and does not fall back to whisper.cpp. The Python child has a 55s default timeout below the 60s desktop IPC cap. Host pending behavior is covered by existing local error tests. |
| AC6 | PASS    | Faster-whisper uses the same sidecar request/status/unload lifecycle surface. Sidecar cancel passes an atomic flag to the faster-whisper runner and kills the Python child process.                                                                                                |
| AC7 | PASS    | `docs-site/concepts/dictation.md` documents default backend, faster-whisper requirements, and no silent fallback.                                                                                                                                                                  |

## Verification Commands

| Command                                                                                                                                                                                           | Verdict | Notes                  |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- | ---------------------- |
| `rtk proxy powershell -NoProfile -Command '$env:CARGO_TARGET_DIR=".tmp\cargo-faster-whisper"; cargo test --manifest-path platform/runtime/Cargo.toml local_sidecar_protocol -- --test-threads=1'` | PASS    | `3 passed`.            |
| `rtk proxy powershell -NoProfile -Command '$env:CARGO_TARGET_DIR=".tmp\cargo-faster-whisper"; cargo test --manifest-path platform/runtime/Cargo.toml dictation::local -- --test-threads=1'`       | PASS    | `30 passed`.           |
| `rtk proxy powershell -NoProfile -Command '$env:CARGO_TARGET_DIR=".tmp\cargo-faster-whisper"; cargo test --manifest-path platform/runtime/Cargo.toml dictation::host -- --test-threads=1'`        | PASS    | `56 passed`.           |
| `rtk proxy powershell -NoProfile -Command '$env:CARGO_TARGET_DIR=".tmp\cargo-faster-whisper"; cargo test --manifest-path platform/runtime/Cargo.toml faster_whisper -- --test-threads=1'`         | PASS    | `4 passed`.            |
| `rtk proxy bun run --cwd platform/desktop typecheck`                                                                                                                                              | PASS    | `tsc --noEmit` passed. |

## Notes

- This slice does not install `faster-whisper` or convert ggml `.bin` models to
  CTranslate2 format.
- `KOSMOS_FASTER_WHISPER_PYTHON` can point at a specific Python executable.
- Default `python` on the tested Windows machine did not have `faster_whisper`
  installed, so a real faster-whisper run requires an explicit Python env or
  packaging work.
