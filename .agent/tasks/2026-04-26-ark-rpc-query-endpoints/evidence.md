# Evidence: ARK RPC query endpoints

## Verification status

PASS

## Acceptance criteria

- AC1 PASS: `ark-core-rpc` exposes `list_objects_by_type` and `get_objects_by_ids`.
- AC2 PASS: `ark-core-rpc` exposes `list_recent_usage_processes` and `search_usage_processes`.
- AC3 PASS: `@kepler/ark` calls the new RPC operations directly for object type/id queries and usage process queries.
- AC4 PASS: Arrancador process search calls `ArkUsageApi.processes.recent/search` before falling back to read-only SQLite.
- AC5 PASS: ARK core, SDK, Arrancador boundary docs, and TODO mention the runtime query endpoints.
- AC6 PASS: Fresh checks passed against isolated test/smoke databases.

## Code changes verified

- `packages/ark-core/rust/src/db.rs`
  - Added Rust SQLite query helpers for object type/id reads.
  - Added Rust SQLite query helpers for recent and searched usage process candidates.
  - Added focused tests for the new query helpers.
- `packages/ark-core/rust/src/main.rs`
  - Added RPC request variants and handlers for the four query operations.
- `packages/ark-core/rust/src/types.rs`
  - Added `UsageProcessCandidate`.
- `packages/kepler-ark/src/ark-client.ts`
  - Rewired `objects.listByType`, `objects.getMany`, and new `usage.processes` APIs to RPC calls.
- `apps/arrancador/electron/main/services/usage-process-search.ts`
  - Uses ARK runtime process queries first, with read-only SQLite as fallback.

## Commands

- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml object_query_helpers_filter_by_type_and_ids`
- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml usage_process_queries_return_recent_and_search_candidates`
- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml`
- `bun run --cwd packages/kepler-ark typecheck`
- `bun run --cwd apps/arrancador test`
- `bun run --cwd apps/arrancador typecheck`
- `bun run ark:smoke`
- `git diff --check`

All listed verification commands passed. `bun run ark:smoke` uses the repository smoke matrix with isolated test/smoke databases.
