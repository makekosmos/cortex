# ARK storage boundary cutover

This document records the selected design for removing SQLite ownership from `ark-core` while preserving the existing Engine and wire behavior.

## Decision

`ark-core` keeps command admission, source identity, queue limits, fairness, cancellation, the maintenance barrier, and Engine-facing error mapping. `ark-store` owns the production SQLite connection, SQL, read connections, transaction and savepoint execution, schema bootstrap, compatibility and canonical migrations, snapshots, restore, and recovery.

`ark-core` command bodies use typed application capabilities. Store creates each capability inside a synchronous read, write, or maintenance callback. The callback receives named operations such as object, task merge, sync, usage, registry, and integration persistence. It never receives a `Connection`, `Transaction`, statement, SQL string, raw row type, or generic query function. The callback cannot await. Existing read admission and maintenance barriers stay outside the database operation and retain their current ordering.

Each existing command keeps its current transaction scope. Multi-row operations that update a domain row, revision or vector, receipt or journal, and committed feed remain atomic. Check-then-write rules read and write through the same capability. A transaction-scoped value cannot outlive its callback. Network work happens after the transaction ends.

`ark-sync` owns synchronization policy and connected-session state. `ark-net` owns sockets and persistent Iroh identity transport. `ark-domain` owns pure rules and domain types. `ark-application` owns behavior-oriented persistence contracts and use cases. `ark-core` composes those packages and preserves the Engine JSON and protocol shapes.

## Compatibility

The required compatibility contract is the public Engine API and serialized wire behavior. Current in-workspace consumers also use `ark_core::db` from runtime code and integration tests. Before deleting those Rust exports, migrate the production `canonical_app_key` caller to its current Store implementation and move the integration-test database inspection helpers to Store test support. Then remove or narrow each Core export with its callers in the same change. No compatibility wrapper may accept SQLite types or retain SQL in Core.

`ark-core` ends with no normal or development `rusqlite` dependency. Core tests that need persistence behavior use Store-owned fixtures or exercise Engine commands and inspect results through a Store-owned test API. A final source and Cargo graph gate rejects SQLite imports, SQL execution, database opening, and SQLite dependencies in Core.

## Cutover order

1. Capture the current Cargo dependency edge, Core SQL and SQLite call sites, public exports, and the transaction/savepoint behavior of every command, query, maintenance operation, and bootstrap phase. Add the final ownership gate with fixture tests that prove it catches the current leak.
2. Replace raw SQLite in the actor task contract with Store-owned opaque read, write, and maintenance capabilities. Keep Core scheduling and the one-writer invariant. Move connection opening, reader connection ownership, schema bootstrap, snapshot recovery, and maintenance execution to Store in the same storage-lifecycle unit.
3. Move command families with their callers and delete old SQL implementations in each unit. Start with object and usage writes, where application session contracts already express transaction behavior. Continue through task merge, sync feeds and vectors, device registry, and integration replication. Preserve the same transaction scope for every compound operation.
4. Move canonical migration SQL, ledgers, inventories, checkpoints, and task rewrite into Store after the old and target rows, failure points, and transaction boundaries are mapped. Keep pure mapping and validation rules in Domain or Application. Preserve startup order, backup behavior, checkpoints, and recovery semantics.
5. Remove Core SQL facades, direct DB helpers, obsolete adapter modules, and all `rusqlite` dependencies. Migrate every in-workspace Rust caller. Verify Engine JSON and protocol fixtures before the full platform matrix.

Each unit ends with a production-path behavior test, rollback or recovery evidence where the unit writes multiple rows, a clean diff, and a commit. Do not mark an intermediate helper move complete while Core still owns its SQL caller or exposes the raw database boundary.

## Verification predicate

The storage boundary is complete only when all of the following hold on one final commit:

- `ark-core` has no `rusqlite` dependency, SQLite imports or calls, SQL statements, raw row decoders, or SQLite-bearing facade.
- `ark-store` owns the sole production writer and all production SQLite lifecycle and SQL behavior. Store can build and test without importing Core.
- Core retains the accepted admission, fairness, cancellation, reader-barrier, and maintenance ordering behavior.
- Existing Engine JSON requests and responses, protocol serialization, selective-sync narrowing and widening, task merge, replication, snapshot recovery, and migration fixtures pass without output or persisted-data changes.
- The full local gate, independent review, desktop and mobile CI, and supported Windows recovery matrix pass on the exact final SHA.
- Linear acceptance records match the verified commit and test evidence.

## Candidate review

Two GPT-6-Luna design candidates were compared, and a separate GPT-6-Luna cross-judge reviewed them. Candidate B is the base because it preserves Core-owned scheduling and keeps SQLite primitives private to Store. Candidate A's scheduler move to Application conflicts with the accepted single-writer decision in `ark-stage2-boundaries.md`.

The selected shape takes Candidate B's opaque, behavior-named Store operations and transaction ownership. It takes Candidate A's exhaustive command inventory, explicit bootstrap phase order, and requirement to keep synchronous transaction callbacks. It rejects generic SQL or raw transaction access, a monolithic Store command dispatcher, and a new layer of per-table repository traits. Candidate B's signatures remain sketches until the current Application contracts and every transaction scope are reconciled against the source.

The judge found that the actual current branch is `codex/ark-stage2-boundaries`, not the branch named in the dated remaining plan. The plan remains the task sequence; the accepted ownership record and current source are authoritative for this checkout.

## Current baseline

At commit `b209e767`, `cargo tree -p ark-core -e normal --depth 1` shows a direct production edge from Core to `rusqlite`. The production actor still calls `ark_store::writer::SqliteWriter::immediate` with a callback that gives Core commands a raw `Transaction`. Core startup also opens the database, initializes schema, and runs the legacy task rewrite. `cargo test -p ark-core --no-run` succeeds, which confirms the current command tree builds but does not prove the boundary.

The cross-judge requires a complete operation and transaction census before the API contract is frozen. The current checkout has 119 `rusqlite::` source matches across 92 files when paths named as tests are excluded; this search still includes inline test modules and is only a navigation count, not a production count. A source-aware inventory is the next implementation unit.

The fixture-tested gate in `scripts/check-ark-core-storage-boundary.mjs` currently finds 1,278 SQLite references. Its source scanner separates 630 production findings, 635 standalone test-file findings, 11 inline-test findings, and 2 debug-only findings. It scans tests along with production files so the final Core source tree can contain no SQLite fixtures or SQL. These are source matches, not distinct database operations.

The typed command layer has 45 `DbCommand` writes, 24 `DbQuery` reads, and one `DbMaintenanceCommand`. `TypedCommand` runs inside `SqliteWriter::immediate` and maps begin, operation, and commit failures separately. `DbHandle::query` takes read admission, acquires the reader barrier, opens a read-only connection, runs a deferred transaction, and commits it before returning. Maintenance waits for an exclusive reader barrier and calls `with_connection_mut`. The actor runs restore recovery, opens the database, initializes schema, rewrites legacy tasks, then publishes readiness. These source contracts define the transaction and startup behavior that the Store cutover must preserve.

Production findings cluster in `canonical_types` (324), `db` (283), `integration_replication` (8), `type_registry` (8), and `service` (4), with isolated Core transport and relay references. A separate source scan finds 64 savepoint mentions across non-test-named paths; this count can still include inline tests. The generated inventory at `.audit/ark-core-storage-census-2026-10-19.md` lists all 70 typed operations and their execution boundaries. Map each operation to its policy, persistence capabilities, nested savepoints, and post-commit effects before finalizing the Store API. The generated inventory does not infer policy ownership or discover indirect SQL hidden behind helper calls.

## Applied pstack decisions

The **Model the Domain** principle keeps transaction-scoped operations grouped by persisted behavior instead of introducing one repository for every table. The **Sequence Work into Verifiable Units** principle requires each caller migration to finish with its behavior check and deletion of the old SQL path. **Migrate Callers Then Delete Legacy APIs** means no Core SQL facade stays as a compatibility fallback after its consumers move. **Prove It Works** makes the final source and Cargo graph gates necessary but insufficient without the Engine, wire, transaction, and recovery behavior checks.

## Capability API sketch selected on 2026-10-20

Two independent designs and a cross-judge selected the callback-based capability API. `ark-store` owns immediate writes, deferred reads, and exclusive maintenance scopes. Each callback receives short-lived, synchronous types with behavior-named methods. Core keeps actor scheduling, admission, fairness, cancellation, and reader-barrier ordering. No callback exposes a connection, transaction, statement, SQL, row type, or generic query closure.

```rust
pub struct WriteCaps<'tx> { /* private transaction-scoped capabilities */ }
pub struct ReadCaps<'tx> { /* private read-transaction capabilities */ }
pub struct MaintenanceCaps<'scope> { /* private lifecycle operations */ }

impl Store {
    pub fn write<T, E>(
        &mut self,
        operation: impl for<'tx> FnOnce(&mut WriteCaps<'tx>) -> Result<T, E>,
    ) -> Result<T, StoreTransactionFailure<E>>;

    pub fn read<T, E>(
        &self,
        operation: impl for<'tx> FnOnce(&ReadCaps<'tx>) -> Result<T, E>,
    ) -> Result<T, StoreReadFailure<E>>;
}

impl WriteCaps<'_> {
    pub fn usage(&mut self) -> UsageWrites<'_>;
    pub fn objects(&mut self) -> ObjectWrites<'_>;
    pub fn sync(&mut self) -> SyncWrites<'_>;
}
```

The signatures are a target shape, not a frozen inventory. Group methods by persisted behavior, then add only groups supported by the operation and transaction census. Store error types preserve begin, operation, and commit stages without carrying `rusqlite::Error` into Core. Core continues to decide whether a returned error fails the actor closed.

Nested savepoints stay private to Store behavior methods. On success Store releases the savepoint. On operation failure it rolls back to and releases the savepoint, preserving the operation error. If rollback or release fails, the returned typed failure records both the operation and cleanup errors. Store generates unique savepoint names for nested scopes. Signed replication returns a typed replay outcome so Core does not inspect SQLite constraint codes.

The first capability cutover covers local usage writes, local object upsert and delete, and local book-cover writes. Their actor tasks call `SqliteWriter::write_with_capabilities`, which starts one immediate transaction and exposes behavior-specific capabilities. Object and book-cover operations call transaction-scoped Store implementations, so object rows, links, receipts, revisions, tombstones, and sync metadata keep their existing atomic boundary. Core still has the older raw transaction path for commands that have not moved, and the debug-only test backend still calls the Store's connection adapter. Usage rollback, local object/sync-state, delete broadcast, receipt rollback, and book-cover revision-failure rollback tests passed after these changes. The dated census has 59 typed operations on this checkout. It is a navigation index only; the operation-by-operation transaction and side-effect map is still required before the full API is frozen.

The generated census at `.audit/ark-core-storage-census-2026-10-19.md` was refreshed on 2026-10-20. It now finds 897 SQLite references and 59 typed operations, down from 1,279 references and 69 operations in the earlier snapshot. The count change reflects committed extractions since that snapshot, not completion of the ownership boundary. In this checkout the counts are 34 writes, 24 reads, and one maintenance command. `commands/usage.rs` and `LocalUsageWriteCommand` from the old inventory do not exist; use `handle/usage.rs` and `LocalUsageWriteTask` as the source of truth.
