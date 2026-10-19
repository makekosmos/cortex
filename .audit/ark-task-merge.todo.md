# Next Stage 2 unit: Task merge persistence

## Ground

- [x] Read the two independent boundary sketches and cross-check them against the current code and transaction flow.
- [x] Choose one storage API shape and record signatures plus invariants before implementation.

## Throughput checkpoint

- Blocking first steps: map current task-merge SQL, caller-owned transaction boundaries, and behavioral tests before any code moves.
- Independent workstreams: n/a; one cohesive migration crosses the same persistence model and transaction boundaries.
- Shared mutable state: serialize edits to `ark-core` and `ark-store` task-merge modules; do not let parallel workers edit these files.
- Smallest safe decomposition: one implementation owner after design; split only if separate Store-local tests or read-only verification can proceed without overlapping edits.

## Implement and prove

- [x] Move journal paging, row decoding, and canonical payload integrity helpers into `ark-store`, leaving admission and sync policy in Core.
- [x] Prove journal pagination, payload verification, task-merge replay, restart, idempotency, dependency handling, and transaction rollback behavior.
- [x] Move conflict-history reads, persisted operation identity validation, and causal-dependency checks into `ark-store`; Core retains transaction coordination and error adaptation.
- [x] Prove valid and incomplete histories, malformed persisted identity, replay, restart, and rollback behavior.
- [x] Move operation replay/collision checks and atomic operation/guard/journal persistence into `ark-store` using a checked prepared-operation value; keep Core conflict-digest admission and materialization orchestration in the existing transaction.
- [x] Prove guarded replay, operation-id collision, remote pre-bootstrap publication policy, and rollback when journal insertion fails.
- [x] Move bootstrap replay checks, payload loading, baseline-operation persistence, and journal publication into Store; keep existing projection validation and materialization in Core.
- [x] Prove bootstrap idempotency, payload collision, pending-operation publication, and rollback when journal insertion fails.
- [x] Update the ownership census, run targeted gates, review the diff, and commit each completed unit.
- [ ] Continue moving the remaining task-merge persistence modules as separate transaction-preserving units.

## Evidence

- Boundary choice and transaction invariants: `.audit/ark-task-merge-boundary.md`.
- Commit `302d3d85` moves journal paging and payload decoding to Store; 34 task-merge tests, Ark Store/Core Clippy, formatting, and diff checks passed.
- Commit `2facb68a` fixes the writer census to classify standalone `tests.rs` files correctly and refreshes counts for already-moved object/type-registry writers; all 8 census tests and all 80 path/category counts pass.
- Commit `ad25f034` moves conflict-history reads and causal-dependency checks into Store; 37 focused task-merge tests, Clippy, standalone Store check, source-size, writer census, and desktop/mobile graph checks passed.
- The operation append slice passes 40 focused task-merge tests, Clippy, formatting/diff checks, source-size, and writer census (8 tests; 80 path/category counts). It keeps replay-before-admission ordering and consumes a prepared value only after Core's conflict-digest policy passes; this slice is pending commit.
- The bootstrap persistence slice passes 43 focused task-merge tests, Clippy, formatting, source-size, and writer census (8 tests; 81 path/category counts). Store owns bootstrap and baseline SQL; Core validates existing object projections before consuming a prepared bootstrap and retains projection materialization. This slice is pending commit.
- Keep local task materialization, bootstrap, inbound/outbound batch persistence, and operation append boundaries open; the completed slices do not satisfy the full Store ownership predicate.
