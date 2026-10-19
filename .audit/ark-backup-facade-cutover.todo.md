# SQLite backup facade cutover

- [x] Confirm `ark-core::db::backup_to_file` has no production callers and production backups use the Store snapshot pipeline.
- [x] Move the raw connection backup primitive into `ark-store::snapshot` test support and preserve its error text.
- [x] Migrate Core unit and integration fixtures; delete the Core function and export.
- [x] Verify affected behavior and quality gates.

Evidence on this worktree:

- `cargo check -p ark-core --all-targets` passed.
- `cargo nextest run -p ark-core snapshot --test-threads 1`: 26/26 passed.
- `cargo nextest run -p ark-core backup_to_file --test-threads 1`: 2/2 passed.
- `cargo nextest run -p ark-core phase3_migration_system --test-threads 1`: 11/11 passed.
- Store all-target Clippy, Core library Clippy, format, source-size, and diff checks passed.
