# Problems and Fixes

## P1: Backup sidecar protocol needed a large payload without external crates

- Problem: crates.io access is unreliable in this environment, so adding `serde` for large backup payloads was not safe.
- Fix: kept the sidecar dependency-free and passed large backup copy payloads through a temporary JSONL request file.

## P2: Manifest parser assumed field order

- Problem: smoke restore failed when a manifest had `originalPath` before `backupPath`.
- Fix: parse each manifest object independently, so field order no longer matters.

## P3: Backup smoke initially restored to original source paths

- Problem: the first smoke test did not prove restore into a separate target directory.
- Fix: rewrote the smoke manifest explicitly and verified restored files exist under a separate `restore/` folder.

## P4: Full Ark/local SQLite migration is not safe as a blind refactor

- Problem: Arrancador has two different SQLite ownership domains: launcher-local `arrancador.db` and selected-space Ark DBs.
- Decision: keep launcher-local `arrancador.db` local for now; document Ark Rust API requirements instead of rewriting persistence without a reversible migration plan.
