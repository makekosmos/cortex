# Evidence

## Result

Verification status: PASS.

All acceptance criteria from `spec.md` passed against the current codebase.

## Baseline vs Final Metrics

| Metric              |     Baseline |  Final | Change |
| ------------------- | -----------: | -----: | -----: |
| Source files        |          209 |    227 |    +18 |
| Code files          |          197 |    215 |    +18 |
| Test files          |           33 |     37 |     +4 |
| Total code lines    |        26088 |  26969 |   +881 |
| Files >300 lines    |           18 |     17 |     -1 |
| Files >450 lines    |            5 |      0 |     -5 |
| IPC commands        |           80 |     80 |      0 |
| Coverage statements | not captured | 80.25% |   PASS |
| Coverage branches   | not captured | 74.73% |   PASS |
| Coverage functions  | not captured | 77.16% |   PASS |
| Coverage lines      | not captured | 81.47% |   PASS |

## Structural Changes

- `LibraryPage.vue` became a route composition surface with `components/library/*` handling toolbar, filters, results, and drop overlay.
- `GameDetailPage.vue` became a thinner orchestration route with shell, section layout, dialogs, and danger-zone UI extracted.
- `ark-usage-backfill.ts` no longer mixes legacy usage planning with database transaction work; planner logic lives in `ark-usage-backfill/plan.ts`.
- `save-root-resolver.ts` no longer owns path token/glob expansion; that moved to `backup/path-expansion.ts`.
- Rust sidecar `main.rs` now only routes JSONL operations; scan, backup/restore, and protocol serialization live in separate modules.
- `scripts/run-e2e.ts` now reliably terminates the Vite preview process tree on Windows after successful Playwright runs.

## Verification Commands

| Command                                                         | Log                            | Result |
| --------------------------------------------------------------- | ------------------------------ | ------ |
| `bun run typecheck`                                             | `final-typecheck.txt`          | PASS   |
| `bun run lint`                                                  | `final-lint.txt`               | PASS   |
| `bun run test`                                                  | `final-test.txt`               | PASS   |
| `bun run test:coverage`                                         | `final-coverage.txt`           | PASS   |
| `bun run build:renderer`                                        | `final-build-renderer.txt`     | PASS   |
| `bun run build:main`                                            | `final-build-main.txt`         | PASS   |
| `bun run build:preload`                                         | `final-build-preload.txt`      | PASS   |
| `cargo test --manifest-path apps/arrancador/sidecar/Cargo.toml` | `final-cargo-test-sidecar.txt` | PASS   |
| `npm.cmd run test:e2e`                                          | `final-e2e-fixed.txt`          | PASS   |

## Acceptance Criteria

- AC1: PASS. IPC command count stayed at 80; no feature category was removed.
- AC2: PASS. Oversized route/service/sidecar hotspots were decomposed into cohesive typed modules/components.
- AC3: PASS. New unit/component tests cover extracted planner, path expansion, library UI, game detail composition, and sidecar modules.
- AC4: PASS. Full verification suite passed.
- AC5: PASS. Raw logs, `final-metrics.json`, this evidence file, and `evidence.json` are present.
- AC6: PASS. No remaining file exceeds 450 lines, all checks pass, e2e passes, and no obvious sub-10 blocker remains in this scope.
