# KOS-424 type mutation application boundary

- [x] Move type registration, object type, and object link use cases with their storage session contract into `ark-application`.
- [x] Use domain `TypeRegistration` and object/sync contracts; keep SQLite implementations in `ark-core`.
- [x] Preserve `ark_core::application::type_mutation` as a re-export facade.
- [x] Run the real SQLite type mutation and object-link service tests (6 passed), application tests, strict Clippy, dependency-tree, format, source-size, and diff gates.
- [ ] Commit independently; keep KOS-424 open.
