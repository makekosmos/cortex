# Evidence

## Result
PARTIAL

## Acceptance Criteria
- AC1 PASS: Arrancador can persist multiple additional bindings per game, skips same-game duplicates, ignores primary-path self-duplicates, and rejects collisions with another game's primary executable. Raw: `raw/bun-tests-final.txt`.
- AC2 PASS: Ark usage hydration and playtime range aggregation now attribute usage by primary `exe_path`, extra `exe_path` bindings, and `process_name` bindings. Raw: `raw/bun-tests-final.txt`.
- AC3 PASS: Main-process running-instance counting and termination now resolve all process matches for a game before acting, covering primary executable plus explicit bindings.
- AC4 PASS: Game detail now exposes a process-bindings section and picker modal with recent-10 loading, debounced search, and multi-select add. Picker state is covered by `src-vue/test/use-usage-process-picker.test.ts`. Raw: `raw/test-final.txt`.
- AC5 PARTIAL: Core verification commands pass, but `bun run test:e2e` is blocked in this environment by Windows `EPERM` when Playwright tries to fork workers and when Chromium itself is spawned. Raw: `raw/test-e2e-final.txt`.
- AC6 PASS: `spec.md`, `problems.md`, `evidence.md`, `evidence.json`, and raw artifacts are present under this task directory.

## Verification Commands
- `bun run typecheck` PASS, raw: `raw/typecheck-final.txt`.
- `bun run test` PASS, raw: `raw/test-final.txt`.
- `bun test electron/main/services/ark-usage-bindings.test.ts electron/main/services/game-process-bindings.test.ts` PASS, raw: `raw/bun-tests-final.txt`.
- `bun run biome:check` PASS, raw: `raw/biome-check-final.txt`.
- `bun run build:renderer` PASS, raw: `raw/build-renderer-final.txt`.
- `bun run build:main` PASS, raw: `raw/build-main-final.txt`.
- `bun run build:preload` PASS, raw: `raw/build-preload-final.txt`.
- `bun run build` PASS, raw: `raw/build-final.txt`.
- `bun run test:e2e` BLOCKED by environment, raw: `raw/test-e2e-final.txt`.

## Notes
- The Rolldown `[PLUGIN_TIMINGS]` warning for `vite:vue` is informational timing telemetry, not a failed build.
- `scripts/run-e2e.ts` now retries via an inline Playwright fallback when the standard Playwright runner cannot fork workers, but this environment also blocks launching Chromium itself.
