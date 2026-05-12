# Command Results

## PASS

- `bun run --cwd apps/delphi/ts test`
  - Result: PASS.
  - Output included: `Test Files 8 passed (8)`, `Tests 99 passed (99)`.
- `bun run --cwd apps/delphi/ts build`
  - Result: PASS.
  - Output included successful Vite renderer/main/preload builds, electron-builder packaging with Electron 38.8.6, and `cargo build --release --manifest-path ../../../packages/ark-core/rust/Cargo.toml --bin ark-core-rpc`.
- `bun run --cwd apps/delphi/ts e2e`
  - Result: PASS.
  - Output included: `1 passed`.
  - The test runs with isolated temporary `KEPLER_TEST_APPDATA` and `KEPLER_TEST_USER_DATA` paths.
- `bun run --cwd apps/eden/ts test:ark-migration`
  - Result: PASS.
  - Output included: `{"status":"ok","objectTypes":["note_obj","research_note"],"objects":["note-a","note-b"],"links":["note-a:related:note-b"],"failureStatus":"partial_failure"}`.
- `bun run --cwd apps/eden/ts build`
  - Result: PASS.
  - Output included successful Heart release build, `ark-core-rpc` release build, TypeScript, renderer, main, and preload builds.
- `git diff --check`
  - Result: PASS.
  - Output included only line-ending warnings.
- `rg -n "Test database isolation|isolated test databases|main/user ARK database" AGENTS.md`
  - Result: PASS.
  - Output confirmed the root `AGENTS.md` invariant.

## Notes

- Earlier Delphi Playwright launch failures were traced to overriding `USERPROFILE` in the test environment. That override was removed.
- Direct SQLite reads from the Playwright test were removed to avoid native `better-sqlite3` ABI coupling; persistence is verified through ARK IPC and Eden visibility.
