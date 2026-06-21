# 2026-06-21 faster-whisper-backend

## Context

Local dictation now routes through `kosmos-local-stt.exe`. The default backend
is `whisper.cpp` through the managed `whisper.dll` sidecar path. We want to keep
that default, but add an optional `faster-whisper` backend that can run the same
selected local model from the user's perspective through a different engine.

## Scope

- Add a local STT backend selector with default `whisper.cpp`.
- Add `faster-whisper` as an optional backend value in config, UI/state
  serialization, sidecar protocol, and docs.
- Keep existing local model selection UX intact: a user can keep selecting the
  same model entry, then choose which backend launches it when compatible.
- Route normal local dictation through the sidecar for both backends.
- Preserve pending queue, cancel, crash isolation, idle unload, accelerator, and
  profile semantics.
- Add tests for config/defaulting, protocol serialization, sidecar routing, and
  fallback/error behavior.

Not in scope:

- Replacing `whisper.cpp` as the default backend.
- Downloading or converting faster-whisper/CTranslate2 models automatically
  unless a minimal metadata hook is required.
- Cloud provider changes.
- Streaming audio redesign.

## Acceptance Criteria

AC1. `whisper.cpp` remains the default local STT backend for existing configs.

AC2. Settings/state expose a local backend option with at least `whisper_cpp`
and `faster_whisper`; selecting `faster_whisper` persists and survives reload.

AC3. Sidecar protocol carries the selected backend, and serialization tests
cover both values.

AC4. Sidecar routing chooses the whisper.cpp implementation for `whisper_cpp`
and the faster-whisper implementation for `faster_whisper`.

AC5. If faster-whisper runtime/model assets are missing or incompatible, the
error is controlled and the pending WAV is kept; the normal path does not
silently fall back to whisper.cpp after the user explicitly selected
`faster_whisper`.

AC6. Cancel/unload/idle behavior applies to faster-whisper requests through the
same sidecar lifecycle surface.

AC7. Docs describe backend selection, default behavior, and current packaging
requirements/limitations.

## Verification Commands

- `cargo test --manifest-path platform/runtime/Cargo.toml dictation::local`
- `cargo test --manifest-path platform/runtime/Cargo.toml dictation::local_sidecar`
- `cargo test --manifest-path platform/runtime/Cargo.toml local_sidecar_protocol`
- `cargo test --manifest-path platform/runtime/Cargo.toml dictation::host -- --test-threads=1`
- Desktop typecheck if Settings UI is touched.
- `bun run docs:sync`
- `bun run docs:check`

## Evidence Requirements

Create `evidence.md` and `evidence.json` with AC verdicts, commands, and any
known unsupported faster-whisper runtime gaps.
