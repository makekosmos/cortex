# Evidence

## Result
PASS

## Acceptance Criteria
- AC1 PASS: Ark usage read-model closes opened SQLite handles in `hydrateGames()` and stats reads; `getPlaytimeStats()` now uses one `getRangeStats()` call instead of concurrent daily/per-game reads.
- AC2 PASS: Arrancador docs now describe Electron + Vue as active, with `src-vue/` as the renderer and no active React/Tauri direction.
- AC3 PASS: Active Arrancador package scripts/dependencies no longer include Tauri or React-specific runtime/test entries; Tauri Rust backend text/config files were removed.
- AC4 PASS: `biome:check` excludes generated/build outputs (`out`, `dist`, `release`, `coverage`, `test-results`, `node_modules`, `src-tauri`) and exits successfully.
- AC5 PASS: Required checks passed in `apps/arrancador`.
- AC6 PASS: Raw command outputs are stored in `raw/`.

## Original Review Findings
- Ark SQLite handles leaked: fixed.
- Active renderer docs still said React: fixed.
- IPC bridge was typed only at compile time: fixed with runtime allowlists.
- `biome:check` checked generated output: fixed.

## Verification Commands
- `bun run typecheck` PASS, raw: `raw/typecheck.txt`
- `bun run test` PASS, raw: `raw/test.txt`
- `bun run biome:check` PASS, raw: `raw/biome-check.txt`
- `bun run build:renderer` PASS, raw: `raw/build-renderer.txt`
- `bun run build:main` PASS, raw: `raw/build-main.txt`
- `bun run build:preload` PASS, raw: `raw/build-preload.txt`
- `bun run build` PASS, raw: `raw/build.txt`

## Notes
- `bun run build` still prints `[PLUGIN_TIMINGS]` for `vite:vue`; this is a Rolldown/Vite timing warning, not a build failure.
- `bun install` was attempted for lockfile regeneration but failed in this sandbox with tempdir `AccessDenied`; see `raw/bun-install.txt`.
