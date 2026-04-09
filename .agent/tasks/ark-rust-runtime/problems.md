# ark-rust-runtime — problems.md (fresh second pass)

Overall verdict: UNKNOWN (deferred-runtime-only, zero FAILs).

## No code fixes remain.

19 of 21 acceptance criteria PASS on a fresh verification pass:

- Rust: `cargo build`, `cargo test` (92 unit + 2 integration), `cargo clippy --all-targets -- -D warnings` all green.
- TS: `bunx tsc --noEmit` clean, no runtime `@arksync/*` imports in `apps/delphi/ts/electron/`.
- Kotlin: `./gradlew :app:assembleDebug` BUILD SUCCESSFUL, `libark_core.so` (arm64-v8a) packaged into the debug APK.
- AC13 is now PASS. The previous blocker (`apps/delphi/ts/electron/delphi-storage.ts` with `import { HLC } from '@arksync/core'`) has been resolved by deleting the file. The only remaining `@arksync/core` reference in `apps/delphi/ts/electron/` is a single type-only `import type { PeerChange, SyncEntity } from '@arksync/core'` in `main.ts:7`, which the AC explicitly permits.

## Remaining non-PASS criteria

Both are UNKNOWN by spec Assumption A6 (deferred-device-test), not FAIL:

- **AC12 — Electron dev run.** Requires live Electron + Android peer on the same LAN. Outside this sandbox. All static prerequisites (ark-core-rpc binary, sidecar event demux, beacon + server + client spawning, IPC wiring) are in place and covered by unit and integration tests.
- **AC18 — Android runtime parity on real device.** Requires a physical arm64 device. Outside this sandbox. Acceptance via AC15 + AC17 + AC21 + the `sync_round_trip` integration test (which exercises the event-callback path end-to-end in-process) is allowed by spec A6.

## Recommendation

No further code changes are needed. To upgrade the overall verdict to PASS, run the two deferred runtime checks on real hardware:

1. `bun run dev` from `apps/delphi/ts/` with the Android Delphi build installed on a physical device on the same Wi-Fi; verify the connection indicator goes green and bidirectional sync works for create/update/delete.
2. Install `app/build/outputs/apk/debug/app-debug.apk` on a Nothing A063 (or similar arm64 Android device), pair it against the Electron instance, and verify beacon discovery + hello handshake + sync both directions.
