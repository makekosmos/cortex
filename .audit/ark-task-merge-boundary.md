# Task-merge storage boundary

## Goal

Move task-merge SQL and SQLite row interpretation out of `ark-core` into `ark-store` without changing Engine/protocol shapes, operation replay behavior, policy decisions, or the command actor's one-immediate-transaction guarantee.

## Grounding

- Core's `DbCommand::execute` runs synchronously inside one immediate transaction created by `ark_store::writer::SqliteWriter`.
- The task-merge command wrappers enforce Engine/sync source identity and registry admission; inbound policy checks read mutable persisted state in the same transaction.
- `ark-core/src/db/task_merge.rs` and its five child modules currently mix SQL/row decoding with replay, conflict and task-merge operation behavior.
- Local task changes can atomically write object rows, links, sync revision, bootstrap, operations and journal entries. Those writes must stay in one actor command and one transaction.
- The domain operation/bootstrap types already live in `ark-domain`; journal/batch DTOs live in `ark-application`. Store depends on both.

## Candidates and review

Candidate A extracted typed SQL/row APIs that borrow the existing transaction. It preserves atomicity with the smallest executor change, but risks leaving a sequence of Core calls that can omit parts of a persisted invariant.

Candidate B routed a closed task-merge operation through the Store writer and removed raw SQLite handles from Core. It better matches the end state, but its proposal did not specify how Core-owned mutable authorization is evaluated against the exact transaction snapshot. A precheck in another transaction is racy; moving the policy into Store violates ownership.

The independent judge scored both 18/25. The judge preferred B as the end state only after defining a synchronous, same-transaction admission seam; it rejected calling A a completed boundary.

## Synthesis decision

Use Store-owned semantic persistence operations, not one method per SQL statement. Initially call them with the transaction already created by `SqliteWriter`; Core remains the synchronous command/admission coordinator and invokes Store's narrow typed reads for mutable persisted facts inside that same transaction. Store owns task-merge SQL, row codecs, and atomic multi-row persistence behavior. Core owns source admission, registry authorization, sync-policy decisions, Engine/API mapping, and protocol orchestration. Pure conflict-set and projection rules remain in `ark-domain`.

This keeps the existing actor's transaction guarantee while removing task-merge SQL and row decoding from Core. It is an incremental boundary within the current executor contract; do not claim the global `ark-core` SQLite dependency is removable until every remaining command/query no longer names SQLite types. Revisit the executor seam only if remaining SQL-free commands still require a direct rusqlite dependency.

## Typed operation shape

The module should expose complete persistence actions and typed results, for example:

```rust
pub fn read_journal_page(
    connection: &Connection,
    after: i64,
    through: i64,
    limit: usize,
) -> Result<TaskMergeJournalPage, TaskMergeStoreError>;

pub fn append_local_operation(
    transaction: &Transaction<'_>,
    operation: TaskMergeOperation,
    expected_conflict_digest: Option<&str>,
) -> Result<TaskMergeAppendResult, TaskMergeStoreError>;
```

Signatures are refined per operation. Do not expose SQLite rows, tuples, SQL strings, generic callbacks, or one-query-per-method ports. Policy checks remain outside these operations but inside the same actor transaction. Store operations must not open or commit nested transactions.

Bootstrap persistence uses `check_bootstrap` followed by `persist_bootstrap`. The check returns either an idempotent replay or a prepared bootstrap. Core checks any existing object projection before it consumes the prepared value. Store writes the bootstrap, baseline operations, and journal entries together. `load_bootstrap` verifies the stored payload before Core uses it to rebuild a missing projection.

Store also owns outbound progress row decoding, the journal watermark query, and conditional insert, policy-stamp, batch-claim, and acknowledgement writes. It returns a typed progress state with optional in-flight and last-acknowledged batches. Core still validates the current sync policy, fits entries to the wire budget, and decides whether a replayed or new acknowledgement is valid.

## Local task materialization decision

The remaining local upsert/delete path must not be moved wholesale from Core into Store: canonical-type validation, expected-snapshot admission, source identity, and explicit conflict-resolution policy stay in Core. It also must not be decomposed into a sequence of Core-issued CRUD calls, because callers could omit one of the coupled object, link, revision, tombstone, bootstrap, operation, or journal writes.

Two candidates were compared against storage ownership, same-transaction atomicity, policy ownership, and API/migration size. Candidate A exposes a typed current-state read and several Store persistence operations (17/20). Candidate B has Core build a closed domain-specific mutation plan and hands it to one Store apply operation (18/20). An independent GPT-6-Luna judge selected B, with the typed snapshot from A as a graft. This honors the requested Luna worker budget; the orchestrator made the final selection.

The selected boundary is a typed `TaskMergeLocalMutationPlan` built from a Store-loaded snapshot inside the current immediate transaction. Store applies the plan through the caller's transaction, rechecks snapshot/conflict guards, persists bootstrap and operation/journal rows, materializes with the pure `ark-domain` task-merge projection, and writes the projection, source links, object revision, version-vector/feed effects, and tombstone state as one semantic operation. Store returns the materialized object and assigned revisions; Core performs canonical validation and builds the existing `SyncEntity` result. No rusqlite rows, SQL strings, callbacks, or transaction ownership cross into Core's plan.

The state read includes the existing object and sorted source links, bootstrap presence, object identity, task snapshot token, and the legacy object-revision fallback. The plan remains task-merge-specific; it is not a generic batch API. Hard delete must continue to remove source links and produce the existing tombstone entity, soft delete must retain the projection, concurrent existence alternatives must remain unresolved absent an explicit digest, and causal dependencies must keep the projection pending. Failure injection must prove all related tables roll back together.

Candidate sketches and the independent score are recorded in the active task transcript; this decision supersedes the earlier open instruction to choose the complete local task materialization seam during implementation.

## Migration sequence

1. Move bounded journal page reading and its payload verification into Store; add Store-owned schema tests for valid pages, invalid cursors/limits, future watermarks, missing/mismatched payloads. Keep Core command/service tests for observable behavior.
2. Move conflict-history and dependency reads, then append/bootstrap and inbound/outbound persistence in cohesive operations. Delete each Core SQL implementation with its callers.
3. Move local task materialization only with object/link/revision/bootstrap/journal persistence as one Store operation, after its exact mutation and failure semantics are tested.
4. Audit the task-merge Core source for production SQL/row codecs; retain only Core admission/policy orchestration and error translation.
5. Re-run Store independence, source-size, writer census, graph, targeted test and affected-workspace gates before beginning another domain.

## Required invariants

- Same operation id and payload replays idempotently; different payload or guard is a collision.
- Conflict digest checks and guard insertion stay atomic with the operation/journal writes.
- Inbound batches validate identity, digest, contiguous positions and replay state before committing any entry; a bad later entry rolls back earlier entries and progress.
- Outbound in-flight batches replay exactly; acknowledgement is monotonic and tied to the exact batch and policy identity.
- Local task object, links, revision, bootstrap, operation and journal writes share one transaction.
- No network call or await occurs while the Store writer holds the transaction.
- Existing Core source/admission/policy and public Engine/protocol behavior remains covered by actor/service/round-trip tests.
