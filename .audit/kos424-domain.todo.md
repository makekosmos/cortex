# KOS-424 domain extraction

## Slice 1: task merge domain

- [x] Confirm `domain/task_merge` has no dependency on SQLite, transport, application, or `ark-core`.
- [x] Move the module and its tests into `ark-domain`.
- [x] Preserve the existing `ark_core::domain::task_merge` import path as a re-export.
- [x] Verify the new crate dependency tree contains only serde, JSON, hashing, and error types.
- [x] Run crate tests, the full `ark-core` library suite, strict Clippy, formatting, diff, and source-size checks.

`cargo test -p ark-domain`: 10 passed. The parallel `ark-core` run hit the existing Iroh policy timing test; it passed alone, and `cargo nextest run -p ark-core --lib --test-threads 1 --no-fail-fast` passed 549/549. Strict Clippy for both crates and all source gates passed.

Remaining extraction: move the other domain contracts only after resolving their type ownership; then extract application, store, sync, and net one proven boundary at a time. This slice does not claim KOS-424 complete.

## Slice 2: canonical types, device registry, and HLC

- [x] Keep canonical type definitions, signed device registry rules, and HLC in the pure domain crate.
- [x] Preserve existing `ark_core::domain::*` and `ark_core::hlc` paths while callers remain in ark-core.
- [x] Move the canonical registration fixture with its tests.
- [x] Run `cargo test -p ark-domain` (38 passed) and `cargo check -p ark-core`.
- [x] Run the full serial ark-core suite (521 passed), strict Clippy, formatting, diff, and source-size checks.
- [x] Confirm `ark-domain` has no SQLite, transport, application, or `ark-core` dependency.

This keeps domain rules together while the database and transport remain in `ark-core`. The existing re-exports are a migration bridge for callers in later KOS-424 slices; they must be removed after those callers move. KOS-424 remains open.
