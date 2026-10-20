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

The active branch is `codex/ark-stage2-boundaries`. Core still has a production dependency on `rusqlite`; remaining `DbCommand`, `DbQuery`, and maintenance handlers can still receive SQLite types. The actor now routes local `core.task` writes through a dedicated task, and `SqliteWriter::apply_local_core_task_write` owns its transaction and operation receipt. Core still owns startup and the other unmigrated command paths.

The generated storage census reports 57 typed operations and 900 SQLite ownership references. It finds 32 `DbCommand` implementations, 24 `DbQuery` implementations, and one maintenance implementation. `OperationReceiptCommand` is test-only but remains in the census. Both local and authenticated `core.task` writes are now Store writer tasks, tracked outside this command census. The census is a source index, not a count of distinct SQL operations, and does not infer policy ownership or indirect helper calls.

`.audit/ark-operation-transaction-map-2026-10-20.md` maps the current typed operations and the extracted task writer path to policy, persistence, transaction scope, and observed side effects. The next API work must use this map and update it when an operation leaves the generic Core command contract.

## Applied pstack decisions

The **Model the Domain** principle keeps transaction-scoped operations grouped by persisted behavior instead of introducing one repository for every table. The **Sequence Work into Verifiable Units** principle requires each caller migration to finish with its behavior check and deletion of the old SQL path. **Migrate Callers Then Delete Legacy APIs** means no Core SQL facade stays as a compatibility fallback after its consumers move. **Prove It Works** makes the final source and Cargo graph gates necessary but insufficient without the Engine, wire, transaction, and recovery behavior checks.

## Selected execution boundary

Two independent designs and a cross-judge selected Application-owned use cases with Store-implemented persistence sessions. Core keeps actor admission, queue fairness, cancellation, reader barriers, maintenance ordering, and Engine error mapping. A writer task calls a named Store operation. Store starts one immediate transaction, constructs short-lived adapters for the Application ports, calls the use case, and commits only after success. Queries follow the same shape with a read snapshot. Store does not expose a connection, transaction, statement, SQL, row type, or generic query callback to Core.

Store may use transaction-scoped capability types internally to group SQL by persisted behavior. Core calls behavior-level writer and reader methods. The public Store surface must not become one method per table or a dispatcher that mirrors every Core command. Store error types preserve begin, operation, and commit failures without carrying `rusqlite::Error` into Core. Core decides whether an error stops the actor.

Nested savepoints stay private to Store behavior methods. On success Store releases the savepoint. On operation failure it rolls back to and releases the savepoint, preserving the operation error. If rollback or release fails, the returned typed failure records both the operation and cleanup errors. Store generates unique savepoint names for nested scopes. Signed replication returns a typed replay outcome so Core does not inspect SQLite constraint codes.

The verified Store-owned writer paths cover local usage writes, local object upsert and delete, local book-cover writes, canonical game upserts, local `core.task` writes, and authenticated `core.task` writes. The local task path keeps its `object.upsert.v1` receipt, task mutation, object readback, and returned commit in one Store transaction. The authenticated task path keeps policy identity, peer admission, inbox receipt, stale handling, task mutation, and inbox acknowledgement in one Store transaction, with orchestration in Application. Actor tests prove rollback after persistence/commit failures; replay, stale payload, policy change, and idempotency regressions pass. Core still has the generic raw transaction path for other operations and retains raw SQLite dependencies. The current census and operation map identify the next cutovers.

## Object read cutover sketch

The first read-family cutover covers all `ObjectReadQuery` variants and keeps Engine request dispatch and JSON serialization in Core. Store owns the read-only connection, deferred transaction, and SQL. The query body receives callback-scoped `ReadCapabilities` with `objects()` and `canonical()` operations; it never receives a connection, transaction, SQL, raw row, or generic query function.

```rust
pub struct SqliteReader { connection: Connection }

pub enum ReadSnapshotFailure<E> {
    Begin(StoreError),
    Operation(E),
    Commit(StoreError),
}

impl SqliteReader {
    pub fn open_read_only(path: &str) -> Result<Self, StoreError>;
    pub fn with_read_snapshot<T, E>(
        &mut self,
        operation: impl for<'snapshot> FnOnce(ReadCapabilities<'snapshot>) -> Result<T, E>,
    ) -> Result<T, ReadSnapshotFailure<E>>;
}
```

Store creates owned result capabilities from the private transaction. Their methods delegate to the existing Store readers. Canonical asset-source validation uses `ark_domain::canonical_type`; the SQL and `AssetSourceError` move from Core to Store. The `CanonicalGame` query continues returning a required `GameRecord`, `WriteSnapshot` continues returning a required `ObjectWriteSnapshot`, and `Get` continues serializing an absent object as JSON `null`.

Core keeps its actor admission permit and reader barrier around `SqliteReader::with_read_snapshot`, then maps begin and commit failures to the existing storage error. The callback maps operation failures through the existing `DbError` path. It remains synchronous, and only owned values may leave the callback. `DbQuery` still has unrelated raw-connection callers during this staged migration; track those as outstanding, and remove the legacy contract when the remaining query families move. Do not add a Store enum that mirrors the Engine query variants.

The design review compared a behavior-capabilities API with a closed Store request/result dispatcher. The cross-judge recommended behavior capabilities (22/25 vs. 18/25): object and canonical capabilities fit the accepted boundary better than a parallel Store enum that mirrors Engine operations. The selected shape also keeps the competing design's explicit Engine-JSON constraint and begin/operation/commit failure checks. The review was design-only; signatures above are reconciled against current Store return types, including required canonical-game and write-snapshot results.

Implementation is in progress in `ark-store::read_snapshot` and `ark-store::read_capabilities`. All `ObjectReadQuery` variants use this Store read path; Core retains JSON shaping and admission/barrier ordering. Canonical asset-source SQL is Store-owned. The only connection-taking entry point is behind the explicit `test-support` feature for Core's debug-only in-memory backend. Store tests prove two reads retain one snapshot while a writer commits and prove operation failures keep their typed stage. At this checkpoint, `ark-store` passed 101 tests, `ark-core` passed 613 tests (one skipped), and targeted Clippy passed. Other Core `DbQuery` implementations still use the legacy connection contract and remain in scope.

The five `UsageReadQuery` variants now use a Store-owned `usage()` capability through the same read snapshot; the Engine RPC layer continues to serialize the same values. Their focused usage set passed 33/33, including the Engine RPC and sync regressions, and Clippy with warnings denied passed. `SystemReadQuery`, type registry, sync, replication, device registry, and task-merge read queries still need the Store capability cutover.
