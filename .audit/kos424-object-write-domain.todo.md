# KOS-424 object write domain contracts

- [x] Move optimistic object-write snapshots, inbound write records, and canonical payload hashing into `ark-domain`.
- [x] Keep `ark_core::domain::object_write` as a module re-export for existing storage and service callers.
- [x] Run object snapshot (3), inbound compatibility (5), and operation-receipt (8) service tests, strict Clippy, dependency-tree, format, source-size, and diff gates.
- [ ] Commit independently; keep KOS-424 open.
