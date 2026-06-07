# Cross-Platform Adapter macOS MVP Evidence

Date: 2026-06-07
Branch: `structural-refactor-ci-guards`
Classification: `FULL_LOOP`

## Summary

Phase 1 is implemented as an adapter-first macOS MVP slice:

- macOS app index source scans standard `.app` bundle locations and launches through `/usr/bin/open`.
- Electron backend resolution and electron-builder resources are platform-aware instead of Windows-only.
- Swift helpers from `sample/SuperCmd-main` are copied under runtime native sources and built into the desktop package temp native directory.
- Runtime owns macOS helper resolution, permission checks, native audio ping, hotkey event bridge, helper lifecycle, and macOS paste injection.
- Packaged `Kosmos.app` starts from `release/mac`, resolves packaged backend/resources, connects ArkClient, and exits without leaking backend/native helper processes.
- Windows behavior is preserved by keeping existing Windows modules behind the current runtime operation paths; no deep Windows refactor was done in this phase.

## Acceptance Criteria

| AC                                           | Verdict       | Evidence                                                                                                                                                                                                                                                                                                                                                                           |
| -------------------------------------------- | ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1. Platform contract exists                | PASS          | Runtime operation surface includes `dictation.native_status`, `dictation.ensure_native_permissions`, `dictation.native_audio_ping`; app index has platform module dispatch with macOS source. UI does not need to branch for these runtime operations.                                                                                                                             |
| AC2. Windows is wrapped, not rewritten       | PASS          | Windows-specific dictation/hotkey path remains the existing `win_hook` path in `dictation/host.rs`; app index keeps Windows source behavior. Changes were additive and platform-gated.                                                                                                                                                                                             |
| AC3. macOS app search works                  | PASS          | Unit tests for synthetic `.app`; runtime smoke found 220 macOS apps and returned Calculator/Safari-related `.app` results over WebSocket.                                                                                                                                                                                                                                          |
| AC4. macOS app launch works                  | PASS          | Runtime smoke launched `/System/Applications/Calculator.app` through `app_index.launch`; backend returned `{ "ok": true }`.                                                                                                                                                                                                                                                        |
| AC5. macOS native helpers integrated         | PASS          | Swift helper sources live under `platform/runtime/native/macos/`; `bun run --cwd platform/desktop build:native:macos` builds them into `platform/desktop/.tmp/native/macos`; packaged app smoke resolves helpers from `Contents/Resources/native/macos`.                                                                                                                           |
| AC6. Dictation/hotkey MVP path wired         | PASS WITH GAP | Runtime has permission checks, hotkey bridge, audio helper command path, dictation state/config operations, lifecycle cleanup, and macOS paste injection. Smoke shows microphone and input-monitoring granted, all helpers available, and `dictation.native_audio_ping` returns `{ "pong": true }`. Full spoken record/transcribe/paste with an API key was not manually verified. |
| AC7. Electron remains transport/shell        | PASS          | Electron changes are limited to backend executable resolution and package resources/scripts. Product/platform logic lives in Rust runtime and Swift helpers.                                                                                                                                                                                                                       |
| AC8. Local verification passes where allowed | PASS WITH GAP | Rust, TS, JS build, helper build, runtime smoke, direct Electron shell smoke, packaged app smoke, `package:mac`, and process-cleanup checks passed. Playwright `_electron.launch` attach remains a harness gap on this macOS run.                                                                                                                                                  |
| AC9. ARK write boundary intact               | PASS          | `bun run ark:guard:writes` passed after the final changes.                                                                                                                                                                                                                                                                                                                         |
| AC10. Evidence recorded                      | PASS          | This file, `evidence.json`, and raw smoke outputs under `raw/`.                                                                                                                                                                                                                                                                                                                    |

## Verification Commands

```bash
cargo fmt
cargo check --workspace
cargo test -p kepler-backend app_index::platform::macos
cargo test -p kepler-backend dictation::macos_native
bun run --cwd platform/desktop build:native:macos
bun run --cwd platform/desktop typecheck
bun run --cwd platform/desktop build:js:shell
bun run --cwd platform/desktop build:backend
bun run ark:guard:writes
bun .agent/tasks/2026-06-07-cross-platform-adapter-macos-mvp/smoke/audio-capture-smoke.mjs
bun .agent/tasks/2026-06-07-cross-platform-adapter-macos-mvp/smoke/macos-runtime-smoke.mjs --launch
bun .agent/tasks/2026-06-07-cross-platform-adapter-macos-mvp/smoke/electron-spawn-smoke.mjs
bun run --cwd platform/desktop package:mac
bun .agent/tasks/2026-06-07-cross-platform-adapter-macos-mvp/smoke/electron-spawn-smoke.mjs --packaged
ps aux | rg 'kosmos-packaged-spawn-smoke|hotkey-hold-monitor|Kosmos.app|kepler-backend|ark-core-rpc|chrome_crashpad_handler'
```

Runtime smoke required sandbox escalation to bind localhost/open Calculator and use a temp data dir:

```bash
bun .agent/tasks/2026-06-07-cross-platform-adapter-macos-mvp/smoke/macos-runtime-smoke.mjs --launch
```

Runtime smoke results:

- WebSocket hello returned `hello_ok`, protocol `1.0.0`, compatibility `exact`.
- Runtime listed all 6 Swift helpers as available.
- `dictation.get_config` returned default `Ctrl+Shift+;`, `triggerMode: "toggle"`.
- `dictation.get_state` returned `state: "idle"`.
- Microphone permission returned `granted: true`.
- Input monitoring returned `granted: true`.
- `dictation.native_audio_ping` returned `{ "pong": true }`.
- `audio-capture-smoke.mjs` ran `warmup -> start -> stop -> exit` and wrote a WAV file (`28886` bytes).
- `app_index.search` for `Calculator` returned `/System/Applications/Calculator.app`.
- `app_index.search` for `Safari` returned a macOS bundle result.
- `app_index.launch` launched Calculator and returned `{ "ok": true }`.

Electron/package results:

- Direct Electron smoke from `dist-electron/main.js` observed boot self-check, backend spawn, macOS app index, and ArkClient connection.
- `bun run --cwd platform/desktop package:mac` built `platform/desktop/release/Kosmos-0.4.2.dmg`.
- Packaged app smoke launched `platform/desktop/release/mac/Kosmos.app/Contents/MacOS/Kosmos` with no `KOSMOS_MACOS_NATIVE_DIR` or `KEPLER_BACKEND_EXE`, proving packaged resources resolve.
- Packaged app smoke observed boot self-check, packaged backend spawn, macOS app index, and ArkClient connection.
- Process check after packaged smoke found no leftover `hotkey-hold-monitor`, `Kosmos.app`, `kepler-backend`, `ark-core-rpc`, or crashpad helper processes.

Lifecycle fix found during verification:

- Packaged smoke initially left `native/macos/hotkey-hold-monitor` alive after app shutdown.
- Fixed by passing `KOSMOS_PARENT_PID` to the helper, adding a Swift parent watchdog, and adding Rust generation-based helper termination.

Playwright e2e result:

- `bunx playwright test --config platform/desktop/playwright.config.ts platform/desktop/e2e/smoke.spec.ts platform/desktop/e2e/dictation.spec.ts` failed all 6 tests by timeout.
- Diagnostic script showed Electron main did start, self-check passed, backend spawned, lock-file was written, app index found 220 apps, and ArkClient connected.
- The remaining issue is Playwright `_electron.launch` attach/harness timeout on this macOS run, not a demonstrated shell/backend startup failure.

## Known Gaps

- Full visual/manual UI interaction was not performed through a human-driven launcher window.
- Full end-to-end spoken dictation with API key was not verified.
- macOS paste injection uses AppleScript/System Events and may require Accessibility/Automation approval.
- DMG packaging works locally but is unsigned/not notarized: electron-builder skipped signing because no valid Developer ID Application identity is installed.
- Playwright `_electron.launch` attach currently times out on this macOS run despite direct Electron/package smoke passing.
- `build` still targets the Windows publish path; macOS local packaging is exposed separately through `package:mac`.
