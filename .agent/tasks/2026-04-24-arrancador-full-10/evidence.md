# Arrancador Full 10/10 Evidence

Task ID: `2026-04-24-arrancador-full-10`

## Verdict

PASS. AC1-AC6 are satisfied against the current codebase and current command outputs.

The score below is a rubric score from the frozen `spec.md`: 10/10 means every measurable condition in that category passed.

| Category     | Before | After | Measurement basis                                                                                                                                                                                                                                                                          |
| ------------ | -----: | ----: | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Architecture |   9/10 | 10/10 | IPC remains thin, route side effects are delegated, backup workflow exposes explicit ports, and architecture boundary tests now guard the backup facade, Game Detail route, and games service splits.                                                                                      |
| Readability  | 8.5/10 | 10/10 | `backup-workflow.ts` went from 499 lines to a 16-line public facade; implementation is split into contracts, internals, shared mapping, read use cases, write use cases, and service composition. `games.ts` went from 395 to 321 lines, with contracts and row-loading helpers extracted. |
| Atomicity    |   8/10 | 10/10 | Backup writes remain centralized in workflow mutation use cases; settings updates stay transactional; existing tests cover ordered backup write flow and missing-backup rejection.                                                                                                         |
| Testing      |   9/10 | 10/10 | Fresh typecheck, unit tests, coverage, e2e, and Rust sidecar tests all pass. Unit count increased from 119 to 121 and coverage stayed above thresholds.                                                                                                                                    |

## Before/After Metrics

| Metric                     | Before |  After |
| -------------------------- | -----: | -----: |
| `backup-workflow.ts` lines |    499 |     16 |
| `games.ts` lines           |    395 |    321 |
| `GameDetailPage.vue` lines |    392 |    392 |
| Unit tests                 |    119 |    121 |
| Coverage statements        | 79.50% | 79.57% |
| Coverage branches          | 73.09% | 74.43% |
| Coverage functions         | 74.52% | 75.31% |
| Coverage lines             | 80.88% | 81.09% |
| E2E tests                  |      4 |      4 |
| Sidecar Rust tests         |      6 |      6 |

## Refactor Evidence

- `apps/arrancador/electron/main/services/backup-workflow.ts` is now a compatibility facade exporting `createBackupWorkflow`, `BackupWorkflow`, and workflow types.
- `apps/arrancador/electron/main/services/backup-workflow/types.ts` owns public ports/contracts.
- `apps/arrancador/electron/main/services/backup-workflow/internals.ts` owns dependency wiring to existing low-level services.
- `apps/arrancador/electron/main/services/backup-workflow/shared.ts` owns shared row mapping, manifest loading, and progress mapping.
- `apps/arrancador/electron/main/services/backup-workflow/read-use-cases.ts` owns query/read workflow methods.
- `apps/arrancador/electron/main/services/backup-workflow/write-use-cases.ts` owns mutation/write workflow methods.
- `apps/arrancador/electron/main/services/backup-workflow/service.ts` composes the workflow.
- `apps/arrancador/electron/main/services/games/service-types.ts` owns games service contracts.
- `apps/arrancador/electron/main/services/games/loading.ts` owns DB row loading and binding hydration helpers.
- `apps/arrancador/src-vue/test/architecture-boundaries.test.ts` now guards these boundaries.

## Verification

| Command                                         | Result                              | Raw artifact                   |
| ----------------------------------------------- | ----------------------------------- | ------------------------------ |
| `bun run typecheck`                             | PASS                                | `final-typecheck.txt`          |
| `bun run test`                                  | PASS, 37 files / 121 tests          | `final-test.txt`               |
| `bun run test:coverage`                         | PASS, 79.57 / 74.43 / 75.31 / 81.09 | `final-coverage.txt`           |
| `bun run test:e2e`                              | PASS, 4 tests                       | `final-e2e.txt`                |
| `cargo test --manifest-path sidecar\Cargo.toml` | PASS, 6 tests                       | `final-cargo-sidecar-test.txt` |

## Acceptance Criteria

| AC                        | Status | Evidence                                                                                        |
| ------------------------- | ------ | ----------------------------------------------------------------------------------------------- |
| AC1 freeze spec           | PASS   | `spec.md` exists and was written before implementation.                                         |
| AC2 split backup workflow | PASS   | Facade is 16 lines; contracts/read/write/wiring/shared/service files exist.                     |
| AC3 preserve behavior     | PASS   | Full unit, coverage, e2e, and sidecar verification passed.                                      |
| AC4 strengthen guardrails | PASS   | Architecture tests now enforce backup facade, Game Detail budget, and games service extraction. |
| AC5 before/after metrics  | PASS   | `baseline-metrics.json`, `final-metrics.json`, and this evidence file exist.                    |
| AC6 fresh verification    | PASS   | All five required commands passed in the final run.                                             |
