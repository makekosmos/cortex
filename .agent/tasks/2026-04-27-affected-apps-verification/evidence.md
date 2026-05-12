# Verification Evidence

Task: `2026-04-27-affected-apps-verification`
Date: 2026-04-27
Result: PASS

## Acceptance Criteria

- AC1: PASS. Shared ARK runtime checks passed.
- AC2: PASS. Arrancador typecheck, unit tests, and packaged smoke passed with isolated test data.
- AC3: PASS. Dashboard typecheck and Electron smoke passed with an isolated smoke database.
- AC4: PASS. Eden ARK migration, production build, and e2e suite passed with isolated test data.
- AC5: PASS. Delphi unit tests, production build/package path, and shared ARK task e2e smoke passed with isolated test data.
- AC6: PASS. Commands, results, isolation notes, and environment limitations are recorded here and in `raw/command-results.md`.

## Final Fresh Checks

- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml`: PASS.
  - 116 library tests passed.
  - 7 `ark-core-rpc` tests passed.
  - Relay and sync integration tests passed.
- `cargo test --manifest-path services\usage-tracker\Cargo.toml`: PASS, 4 tests passed.
- `bun run --cwd packages/kepler-ark typecheck`: PASS.
- `git diff --check`: PASS, no whitespace errors. Git reported LF/CRLF normalization warnings only.

## Application Checks

Arrancador:
- `bun run --cwd apps/arrancador typecheck`: PASS.
- `bun run --cwd apps/arrancador test`: PASS, 47 files and 163 tests passed.
- `bun run --cwd apps/arrancador smoke:packaged`: PASS.
  - Packaged executable started.
  - Smoke used isolated `ARK_DB_PATH`: `D:\Personal\Hobby\Coding\kepler\apps\arrancador\.e2e\packaged-smoke\ark\ark.db`.
  - Smoke used isolated `userData`: `D:\Personal\Hobby\Coding\kepler\apps\arrancador\.e2e\packaged-smoke\localappdata\arrancador`.

Dashboard:
- `bun run --cwd apps/dashboard typecheck`: PASS.
- `bun run --cwd apps/dashboard test:e2e:smoke`: PASS.
  - Smoke database: `D:\Personal\Hobby\Coding\kepler\apps\dashboard\.e2e\smoke-dashboard.db`.
  - Verified database connected status, top app rendering, scrolling, responsive width, and sessions route.

Eden:
- `bun run --cwd apps/eden/ts test:ark-migration`: PASS.
  - Verified object types, objects, links, and partial-failure migration status on isolated test data.
- `bun run --cwd apps/eden/ts build`: PASS.
- `bun run --cwd apps/eden/ts test:e2e`: PASS, 22 tests passed.
  - Tests use isolated temp vaults/app data and test ARK paths.

Delphi:
- `bun run --cwd apps/delphi/ts test`: PASS, 8 files and 100 tests passed.
- `bun run --cwd apps/delphi/ts build`: PASS.
  - Build path compiles the canonical `ark-core-rpc` sidecar and packages the app.
- `bun run --cwd apps/delphi/ts test:e2e -- e2e/shared-ark-task.spec.ts`: PASS, 1 test passed.
  - Verified Delphi task object writes through shared ARK and is visible through Eden using an isolated temp ARK database.

## Isolation

All verification used isolated databases or temporary app data paths. No automated check was pointed at a main/user ARK database.

Arrancador packaged smoke still reports Electron `appData` as the OS roaming directory because that value is an Electron platform path, but the smoke command explicitly used isolated `userData` and isolated `ARK_DB_PATH` for the data under test.

## Remaining Warnings

The successful checks still emit non-failing warnings:
- Electron CSP warnings in e2e/smoke output.
- Vite chunk-size and deprecated Rollup `inlineDynamicImports` warnings.
- Electron-builder metadata/icon/signing warnings.
- Git LF/CRLF normalization warnings.

No acceptance criterion is blocked by these warnings.
