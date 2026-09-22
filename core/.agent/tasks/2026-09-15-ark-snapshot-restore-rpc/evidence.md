# Evidence — KOS-51 atomic ARK snapshot restore RPC

Verified 2026-09-15 on Windows, branch `kos-51`, against the working tree
containing this change.

## AC1 — atomic restore, no missing/partial primary DB

Implementation: `db::restore_snapshot` (`crates/ark-core/src/db/snapshot.rs`)
applies the snapshot via `Connection::restore` (SQLite Online Backup API)
directly into the live `Connection` under the global DB mutex
(`with_conn_mut`). The live `ark.db` file is never renamed, deleted or
replaced — destination pages are written transactionally by the backup API,
so a crash mid-restore leaves the pre-restore state (SQLite journal/WAL
recovery).

- `tests::snapshot_restore::db_backup_restore_roundtrip_survives_reinit` —
  restore via RPC, `restored: true`, `objects: 1`, live objects match the
  snapshot. PASS.
- `db::tests::restore_replaces_live_content_and_stays_valid` — diverged live
  DB fully replaced by snapshot content, `integrity_check` ok after. PASS.
- E2E over the real `ark-core-rpc` binary (`.tmp/kos51-e2e.js` output below):
  `db_backup_restore` returned `{"id":"ark.db.backup-e2e","links":0,"objects":1,"restored":true}`
  and `get_object` afterwards returned the snapshot object. PASS.

## AC2 — backup and restore are serialized

`BACKUP_GATE` (`main/runtime.rs`) is held by the `db_backup` background
thread for the whole chunked copy and by `db_backup_restore` for the whole
restore. Lock order is always `BACKUP_GATE` → DB mutex.

- `tests::snapshot_restore::db_backup_restore_waits_for_backup_gate` —
  gate held for 300ms, restore on a second thread completes strictly after
  release (order assertion `["gate_released","restore"]`). PASS.

## AC3 — invalid/foreign snapshots never touch live data

- `tests::snapshot_restore::db_backup_restore_rejects_invalid_sources_without_touching_live` —
  `../ark.db`, `..\\ark.db`, `sub/dir.db`, dot-staging name, missing file,
  garbage bytes, foreign-schema SQLite → all rejected; `list_objects`
  unchanged, `integrity_check` ok. PASS.
- `db::tests::restore_rejects_traversal_and_missing_ids_without_touching_live`,
  `snapshot_id_validation_accepts_only_plain_basenames` — basename-only id
  contract. PASS.
- `db::tests::validate_reports_integrity_and_schema_verdicts` — garbage →
  `integrity_ok:false`; foreign SQLite → `schema_match:false`. PASS.
- E2E: `{"operation":"db_backup_restore","backup_id":"../ark.db"}` →
  `{"ok":false,"error":"invalid snapshot id: \"../ark.db\""}`. PASS.

## AC4 — restart-equivalent sees snapshot objects

`db_backup_restore_roundtrip_survives_reinit` re-issues `init` on the same
path after restore (drops the connection, reopens the file) and re-reads
`list_objects` — equals snapshot content. PASS.

## AC5 — post-verify failure rolls back to pre-restore state

`restore_snapshot` takes a pre-restore Online Backup into
`backups/.restore-rollback-*` before apply; post-verify failure triggers
`conn.restore` from the rollback file.

- `db::tests::restore_rolls_back_to_pre_restore_state_when_post_verify_fails` —
  injected failing verifier seam; live DB ends in the pre-restore state
  (not snapshot, not corrupt) and `integrity_check` passes. PASS.

## AC6 — list/validate contract

- `db_backup_list_and_validate_contract` — only regular non-hidden files
  listed (subdir + dotfile filtered), typed per-check verdicts. PASS.
- E2E `db_backup_list` → `{"backups":[{"id":"ark.db.backup-e2e","modified_ms":...,"size_bytes":466944}]}`,
  `db_backup_validate` → `{"exists":true,"integrity_ok":true,"schema_match":true,"valid":true}`.
  PASS.

## AC7 — suite and lints

- `cargo test --manifest-path crates/ark-core/Cargo.toml` — fresh run:
  401 passed, 0 failed across all targets. PASS.
- `cargo clippy --manifest-path crates/ark-core/Cargo.toml --all-targets` —
  no warnings in new code (all remaining warnings are pre-existing). PASS.
- `bun run ark:guard:writes` — "ARK write boundary guard passed". PASS.
- `bun run ark:smoke` — rust tests + write-boundary guard pass; the final
  `@makekosmos/ark` published-consumer step requires `GITHUB_TOKEN` and was
  run standalone: `GITHUB_TOKEN=$(gh auth token) node scripts/ark-consumer.mjs`
  → "published @makekosmos/ark@0.1.1 consumer fixture passed". PASS.

## Not verified

- Linux/macOS `open_no_follow` path (product is Windows-only; the fallback is
  `symlink_metadata` + post-open fstat check).
- Interaction with a live sync runtime during restore (RPC mutex blocks all
  DB ops; sync peers apply through the same mutex).
- Cortex-side integration (KOS-27, separate repo).
