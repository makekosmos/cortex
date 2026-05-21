# Dashboard desktop smoke/build check

Date: 2026-04-25
Working directory: `D:\Personal\Hobby\Coding\kosmos\apps\dashboard`

## Scripts found

From `apps/dashboard/package.json`:

- `build:sidecar`: `cargo build --release --manifest-path ../../packages/ark-core/rust/Cargo.toml --bin ark-core-rpc`
- `build:sidecar:dev`: `cargo build --manifest-path ../../packages/ark-core/rust/Cargo.toml --bin ark-core-rpc`
- `rebuild:native`: `electron-rebuild -f -w better-sqlite3`
- `dev`: `bun run build:sidecar:dev && bun run rebuild:native && vite --configLoader native`
- `build`: `bun run build:sidecar && bun run rebuild:native && tsc && vite build --configLoader native`
- `typecheck`: `tsc --noEmit`
- `package:dir`: `bun run build && electron-builder --dir`
- `dist`: `bun run build && electron-builder --win nsis`
- `smoke:seed`: `node --experimental-strip-types scripts/seedSmokeDb.ts`
- `smoke:analytics`: `node --experimental-strip-types scripts/smokeAnalytics.ts`
- `test:e2e`: `bun run build && playwright test`
- `test:e2e:headed`: `bun run build && playwright test --headed`
- `test:e2e:smoke`: `bun run build && node --experimental-strip-types scripts/runE2ESmoke.ts`

## Command results

| Command                   | Result | Key output / error                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `bun run typecheck`       | FAIL   | `electron/services/analytics.ts(4,10): error TS2305: Module '"@arksync/node"' has no exported member 'ArkClient'.`; `electron/services/analytics.ts(4,26): error TS2305: Module '"@arksync/node"' has no exported member 'ArkUsageAnalyticsSnapshot'.`; `../../packages/arksync-node/src/index.ts(1,15): error TS2307: Cannot find module '@kosmos/ark' or its corresponding type declarations.` |
| `bun run build`           | PASS   | Built cargo sidecar `ark-core-rpc`, rebuilt `better-sqlite3`, ran `tsc`, and built Vite outputs for `dist/`, `dist-electron/main.js`, and `dist-electron/preload.mjs`. Warning: `inlineDynamicImports` deprecated; plugin timing warning.                                                                                                                                                        |
| `bun run smoke:seed`      | FAIL   | `better-sqlite3` native ABI mismatch: module compiled with `NODE_MODULE_VERSION 145`, current Node.js v22.16.0 requires `NODE_MODULE_VERSION 127`. Error code `ERR_DLOPEN_FAILED`.                                                                                                                                                                                                               |
| `bun run smoke:analytics` | FAIL   | Node ESM resolution failure: `Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@kosmos/ark' imported from ...\apps\dashboard\electron\services\analytics.ts`.                                                                                                                                                                                                                                  |
| `bun run test:e2e:smoke`  | FAIL   | Build phase passed and Python seeded `.e2e\smoke-dashboard.db`; Electron smoke launched, then failed assertion: `Error: Overview did not expose dashboard status text` at `scripts/runE2ESmoke.ts:52`.                                                                                                                                                                                           |
| `bun run test:e2e`        | FAIL   | Build phase passed and Playwright seeded `.e2e\smoke-dashboard.db`; 3 passed, 1 failed. Failed test: `e2e\dashboard.spec.ts:40:1 › renders overview metrics from Ark DB`; `dashboard-status` did not have non-empty `aria-label` (`Received string: ""` / unexpected `null`). Artifacts in `apps/dashboard/test-results/dashboard-renders-overview-metrics-from-Ark-DB/`.                        |

## Sandbox / environment notes

- Every attempted command inside the default sandbox failed before execution with `windows sandbox: setup refresh failed with status exit code: 1`.
- The same commands were rerun outside the sandbox via approved escalation prompts.
- No network-requiring command was run.
- No GUI/headed command was run. `test:e2e:smoke` and `test:e2e` use Electron/Playwright but not `--headed`; both were able to launch far enough to execute assertions.

## Summary

- Build currently passes.
- Typecheck currently fails on `@arksync/node` / `@kosmos/ark` type resolution/export issues.
- CLI smoke helpers fail before meaningful analytics validation due to native ABI and package resolution problems.
- Desktop smoke/e2e currently fails on missing/non-empty `aria-label` for `data-testid="dashboard-status"`; other e2e checks passed in the full Playwright run.
