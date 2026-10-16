# KOS-423 merge-engine fixture experiment

This is an isolated behavior experiment, not a KOS-423 implementation or a library decision. The standalone package at [`experiments/kos423-merge-harness`](../../experiments/kos423-merge-harness/) pins Automerge 0.12.0, Loro 1.16.2, and Yrs 0.28.0 in its own `Cargo.lock`; its nested `[workspace]` keeps it out of Cortex's workspace. Reproduce from the repository root:

The experiment package and initial report were added in commit `b758db43`.

```sh
cargo test --locked --manifest-path experiments/kos423-merge-harness/Cargo.toml
cargo run --locked --manifest-path experiments/kos423-merge-harness/Cargo.toml
```

The input fixture copies the canonical `core.task` ingress shape from `core/crates/ark-core/src/service/tests.rs:165-193` (including the Tiptap text leaf `hello`), the stable checklist item shape from `core/crates/ark-core/src/db/tests_core/part6.rs:3-16`, and the legacy TodoItem `project_id` relation represented by `core/crates/ark-core/src/types/part01.rs:10-33`. The harness exercises those values through each engine's native map/list/text API. It flattens `state.kind` and `planning.date` into map keys and checklist entries into ID strings; it does not implement a Cortex JSON/Tiptap adapter or a SQLite integration test.

## Results

Both tests pass. Each engine converges compatible concurrent status/date changes independent of update arrival order, retains both concurrent checklist IDs, merges two insertions at the same rich-text position, preserves the common starting state for inspection, tolerates duplicate update delivery, and round-trips its own serialized document. Text insertion order differs between engines, but is stable under either delivery order within each engine.

| Engine           | Concurrent `priority` result              | Divergent `title` / text                                        |
| ---------------- | ----------------------------------------- | --------------------------------------------------------------- |
| Automerge 0.12.0 | Both scalar variants readable (`get_all`) | Both scalar title variants readable; text includes both inserts |
| Loro 1.16.2      | One map winner                            | One title map winner; text includes both inserts                |
| Yrs 0.28.0       | One map winner through ordinary map read  | One title map winner; text includes both inserts                |

The harness prints one payload size per run, but these are tiny artifacts of different types (Automerge save, Loro snapshot, Yrs V1 state update), so they do not compare storage efficiency or production database/network cost. Automerge is the strongest candidate here for a prototype of scalar conflict visibility; this result is not evidence to select or adopt it. Its text behavior also merges both concurrent insertions, rather than preserving two complete rich-text alternatives.

All three preserve a project tombstone flag alongside a concurrent project-name update and the task's `project_id=project-1`. That leaves the link semantically unresolved when reads exclude deleted projects. The experiment establishes that the CRDT libraries do not enforce Cortex cross-object constraints or define whether an update to a deleted object wins, remains hidden, or restores it. The tombstone boolean is only a synthetic operation shape; production uses `deleted_at`/sync tombstones.

The stale-proposal check uses each library's document-wide version token (Automerge heads, Loro frontiers, Yrs state vector) and rejects a token after a later edit. These tokens are deliberately conservative and do not identify whether the _conflict set_ changed; they are not a substitute for KOS-423's acceptance rule. Each format also introduces its own encoding, actor/client identity, history/frontier and compaction behavior, wire compatibility, and database migration questions. None is a drop-in replacement for the existing canonical JSON and sync entity contracts.

## Production gap and next contract

KOS-423 remains open. Cortex already has a guarded `UpsertObject` path using an `ObjectWriteSnapshot`; its tests reject stale object snapshots (`core/crates/ark-core/src/service/tests/object_write_snapshot.rs:22-36,57-99`). That guard is object-wide and does not represent a merge conflict set or a human/AI resolution proposal. `SyncEntity` carries JSON data, HLC, and deletion metadata (`core/crates/ark-core/src/types/part03.rs:21-35`), but no conflict-set identity or resolution envelope.

The minimum next production step is to define a conflict-resolution request/record contract before choosing a library: object ID plus merge unit/field path; common ancestor reference; stable set of competing variants (including deletion); digest/version of that exact set; and an explicitly selected resolution. A SQLite transaction must compare the supplied digest against the current local conflict set and reject if it differs, then atomically store the resolution and append sync delivery metadata. Preserve the existing per-person/device database boundary. HLC may order metadata but cannot resolve conflicts. AI may propose a resolution, but must never apply it without explicit acceptance. Cross-object link/deletion validation and revalidation at commit must be defined in the same domain contract.

After that contract is agreed, add a real `ark-core` SQLite/API test that creates two concurrent object revisions, queries accessible variants and ancestor, changes one variant, then proves an old resolution proposal is rejected transactionally. This experiment does not implement that API, schema, migration, or sync wire change.
