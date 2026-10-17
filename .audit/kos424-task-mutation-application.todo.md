# KOS-424 task mutation application boundary

- [x] Move the object-write storage port and receipt contracts into `ark-application`.
- [x] Move local/remote task mutation use cases into `ark-application`; keep schema preparation and SQLite adapter in core.
- [x] Preserve `ark_core::application::{objects,task_mutation}` Rust paths as re-exports.
- [x] Run task sync (23), object revision (5), and snapshot (3) behavior tests, strict Clippy, dependency-tree, format, source-size, and diff checks.
- [ ] Commit independently and keep KOS-424 open.
