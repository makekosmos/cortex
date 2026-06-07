# Cross-Platform Adapter macOS MVP

## Classification

`FULL_LOOP`

## Goal

Introduce an adapter-first platform architecture for Kosmos and make the first macOS MVP path work for app search, app launch, and dictation/hotkey plumbing while preserving current Windows behavior through thin adapters.

## Context

The desired architecture is:

```text
Vue UI
  -> stable product API
  -> Electron preload/IPC as thin transport
  -> Rust runtime owns domain logic, state, lifecycle, retries, events
  -> OS-specific platform adapters
       windows: existing implementation wrapped first
       macos: Swift helpers adapted from sample/SuperCmd-main
```

Reference planning document: `.omx/specs/cross-platform-adapter-first.md`.

Phase 1 deliberately sets the contract first but does not deeply clean Windows internals. Windows cleanup is Phase 2.

## In Scope

- Runtime-level platform contracts for app search/launch and dictation/hotkey primitives.
- macOS app search/launch implementation.
- macOS native helper build/package path for Swift helpers needed by dictation/hotkeys.
- Thin Windows adapter that preserves existing Windows behavior through the new contract.
- Electron/main routing changes required to call runtime/platform contracts instead of adding new OS-specific product logic in UI.
- Basic macOS local build/dev path for the MVP.

## Out Of Scope

- Focus mode/blocker parity on macOS.
- usage tracker/activity dashboard parity on macOS.
- Full disk file search parity on macOS unless directly needed for app search.
- Clipboard history.
- Public notarized release.
- Auto-update.
- Deep Windows cleanup or behavior redesign.
- Bumping app versions or publishing releases.

## Acceptance Criteria

**AC1. Platform contract exists.** Runtime exposes a stable platform capability surface for app search, app launch, dictation permission/state, and hotkey/dictation control without requiring UI or renderer code to branch on `win32`/`darwin`.

**AC2. Windows is wrapped, not rewritten.** Windows app search/launch and existing dictation/hotkey behavior remain reachable through the new contract with minimal adapter glue, and existing Windows-specific modules are not broadly refactored in this phase.

**AC3. macOS app search works.** On macOS, runtime can enumerate/search installed applications from normal macOS application locations and return launcher-ready records with stable ids, names, paths, and launch metadata.

**AC4. macOS app launch works.** On macOS, runtime can launch a selected app result using macOS-native launch behavior.

**AC5. macOS native helpers are integrated as runtime-managed executors.** Swift helper sources needed for dictation/hotkeys are present under the Kosmos tree, have a build path, and are resolved by runtime/package code as native executors rather than being called directly from UI.

**AC6. Dictation/hotkey MVP path is wired.** Runtime has macOS-capable plumbing for microphone permission, hold/toggle hotkey monitoring, audio capture, and a dictation session lifecycle sufficient for the UI/Electron layer to start/stop dictation without knowing the OS.

**AC7. Electron remains transport/shell.** Any new Electron main changes for these capabilities are limited to transport, process spawning/bootstrap, or window/shell concerns; product lifecycle and platform selection live in runtime/platform code.

**AC8. Verification passes where local environment allows.** At minimum, run:

```bash
cargo check --workspace
bun run --cwd platform/desktop typecheck
```

If macOS build/dev smoke or manual dictation/app-launch checks cannot be completed in the current environment, document the exact gap in `evidence.md`.

**AC9. ARK write boundary remains intact.** If changes touch data-layer paths, `bun run ark:guard:writes` passes or the reason it was not relevant is documented.

**AC10. Evidence is recorded.** `evidence.md` and `evidence.json` record each AC verdict with commands/manual checks and known gaps.
