# KOS-424 authenticated entity storage port

- [x] Confirm the port uses only domain entities and application sync contracts.
- [x] Move the port definition to `ark-application` without changing methods or defaults.
- [x] Keep both `ark_core::AuthenticatedEntityBackend` and
  `ark_core::replication_storage::AuthenticatedEntityBackend` as the same trait.
- [x] Run application/core tests, strict Clippy, dependency and source-size gates.
- [x] Commit this boundary independently; keep KOS-424 open.

The port receives ordinary entities after peer authentication. Its policy-bound
receipt and apply methods use `ark-application::sync_storage` values, while
entities use `ark-domain`. Storage adapters remain in `ark-core` for now.
