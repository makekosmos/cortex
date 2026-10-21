# ARK task-merge local mutation boundary

## Frame

- [x] Read the current upsert, delete, bootstrap, materialization, snapshot, revision, and writer paths.
- [x] Define the acceptance rubric: no Core SQLite in this path; one caller-owned transaction; authorization/canonical rules remain Core-owned; coupled projection and sync writes cannot be omitted; API remains closed and understandable.
- [x] Compare two distinct Store boundary shapes and obtain an independent score.

## Design

- [x] Select a Core-prepared task-specific mutation plan applied by one Store semantic operation; retain a typed Store-loaded snapshot from the alternative.
- [x] Write concrete public/internal type signatures and data invariants before implementation.
- [x] Verify the plan does not move canonical admission, requested-snapshot policy, or explicit conflict policy into Store.

## Implement and verify

- [x] Store loads typed local task state, including sorted links, all unit conflict sets, bootstrap state, snapshot token, and legacy revision fallback.
- [x] Store materializes task history and owns projection repair, links, object revision, and tombstone persistence.
- [x] Replace the remaining Core storage calls with the final plan/apply operation. Core loads typed state from Store, plans changed causal units, and submits one `TaskMergeLocalMutationPlan`.
- [x] Store applies local bootstrap, task operations, journal, projection, links, version-vector/feed, object revision, and tombstone changes in the existing command transaction.
- [x] Core uses the returned materialized object and revision values to preserve current `SyncEntity` output.
- [x] Test stale snapshots, bootstrap-only identity, pending causal dependencies, concurrent alternatives, explicit resolution digest, soft/hard delete, link revisions, and full rollback on injected persistence failure. Current focused tests: Core task-merge 32/32, Store task-merge 22/22; Core rollback coverage includes the production local mutation command with injected journal failure.
- [x] Remove superseded Core SQL and keep every touched source below the hard size limit.
- [x] Run focused Store/Core tests, Clippy, format, source-size, writer census, and graph checks.
- [x] Thermo-nuclear code-quality review; resolve confirmed blockers.
- [ ] Update the ownership evidence and commit only this unit.

## Decision rubric and result

| Criterion | Typed state plus Store persistence calls | Core plan plus one Store apply |
| --- | ---: | ---: |
| Removes SQL ownership from Core | 4/5 | 5/5 |
| Preserves transaction and atomic invariant | 4/5 | 5/5 |
| Keeps policy/auth in the right layer | 5/5 | 5/5 |
| API size and migration risk | 4/5 | 3/5 |
| Total | 17/20 | 18/20 |

Selected the second shape. The smaller A API is grafted as a typed state read with task snapshot token, sorted links, bootstrap presence, and legacy revision fallback. Store receives no generic callbacks or raw SQLite values from Core.

The first implementation unit extracted the typed state read and task snapshot token. Focused Store/Core task-merge checks passed 48/48, Clippy passed, the writer census matched all 81 path/category counts, and the source-size gate passed with the existing grandfathered baseline.

The typed state read now also returns all unit conflict sets from Store. Core uses those values for the same deterministic head and conflict decisions without querying SQLite. The focused task-merge suite passes 49/49 and targeted Clippy passes; the Store state test asserts all ten baseline conflict sets and the task snapshot token are present.

Store now assigns local object and link revisions and writes their version-vector, committed-feed, object-version, and tombstone rows. Four Store revision tests and 49 task-merge tests pass; targeted Clippy, the 81-entry writer census, source-size gate, and desktop/mobile dependency graph checks pass.

The revision fallback now calls `ark-store::sync_versions::object_revision`; the Core implementation and re-export are deleted. Existing revision and stale-write behavior passed in 57 focused Store/Core tests. The legacy local-HLC regression moved into its own test module to keep its parent below the 500-line gate.
