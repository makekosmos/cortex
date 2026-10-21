# KOS-424 sync application ports

## Contract

Move the shared sync use-case ports and value types from `ark-core` into `ark-application`. Keep SQLite setting keys in core. The application crate may depend on `ark-domain`, async-trait, serde, and serde_json, but not on storage, transport, runtime, or `ark-core`.

## Checklist

- [x] Move committed-feed, entity read/mutation, peer progress, and outbound progress contracts into `ark-application::sync_storage`.
- [x] Move `ReplayNotice` to the application contract and preserve its event payload through the existing `ark-core::canonical_types::pending` re-export.
- [x] Keep existing `ark_core::application::sync_storage::*` paths and SQLite setting keys available.
- [x] Add a wire-shape test for replay notices.
- [x] Run application tests (1 passed), full serial ark-core tests (521 passed), strict Clippy, dependency-tree, format, source-size, and diff gates.
- [ ] Commit this boundary independently; keep KOS-424 open for store, sync, and net extraction.
