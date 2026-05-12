# Evidence: ARK usage playtime summary endpoint

## Verification status

PASS

## Acceptance criteria

- AC1 PASS: `ark-core-rpc` exposes `get_usage_game_playtime_summary` and accepts game/process bindings plus optional date range.
- AC2 PASS: `@kepler/ark` exposes `usage.gamePlaytime.summary(...)` with typed bindings and result records.
- AC3 PASS: Arrancador `createGameUsageReadModel` calls the new SDK method before read-only SQLite fallback.
- AC4 PASS: Arrancador `createPlaytimeStatsRepository` calls the new SDK method for range stats before read-only SQLite fallback.
- AC5 PASS: Read-only SQLite remains as a runtime compatibility fallback only.
- AC6 PASS: Docs mention the endpoint and the repository keeps the isolated test database rule.
- AC7 PASS: Fresh verification passed on isolated test/smoke databases.

## Code changes verified

- `packages/ark-core/rust/src/types.rs`
  - Added playtime binding and summary DTOs.
- `packages/ark-core/rust/src/db.rs`
  - Added Rust-side game playtime aggregation by app-provided bindings.
  - Added unit coverage for multi-binding playtime summaries and date range totals.
- `packages/ark-core/rust/src/main.rs`
  - Added the `get_usage_game_playtime_summary` RPC operation.
- `packages/kepler-ark/src/ark-client.ts`
  - Added `usage.gamePlaytime.summary(...)`.
- `apps/arrancador/electron/main/services/ark-usage.ts`
  - Uses ARK runtime aggregation first for game hydration and range stats.
  - Keeps read-only SQLite only as fallback.
- Docs updated:
  - `packages/kepler-ark/README.md`
  - `packages/ark-core/README.md`
  - `docs/ARK-READONLY-SQL-BOUNDARY.md`
  - `apps/arrancador/AGENTS.md`
  - `TODO.md`

## Commands

- `cargo fmt --manifest-path packages\ark-core\rust\Cargo.toml`
- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml usage_game_playtime_summary_matches_bindings_and_range`
- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml`
- `bun run --cwd packages/kepler-ark typecheck`
- `bun run --cwd apps/arrancador test ark-usage`
- `bun run --cwd apps/arrancador test`
- `bun run --cwd apps/arrancador typecheck`
- `git diff --check`
- `bun run ark:smoke`

All verification commands passed after rerunning `bun run ark:smoke` outside the sandbox. The first sandboxed smoke run reached the Playwright step and failed with `spawn EPERM`; the rerun passed and used isolated smoke/test databases.
