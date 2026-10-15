# ARK write-flow census (bounded)

This source-based census covers known syncable writes and their current owners. It is deliberately bounded; it does not classify every SQL mutation in `ark-core`. The backend module extraction changes file ownership only.

| Entry point | Physical write and transaction boundary | Sync metadata coupling | Exception / next owner |
|---|---|---|---|
| Local legacy RPC handlers ([runtime/legacy.rs](../core/crates/ark-core/src/service/runtime/legacy.rs), [runtime.rs](../core/crates/ark-core/src/service/runtime.rs#L157)) | Runtime handlers mutate legacy rows inside `with_write_tx`; see `service/runtime/legacy.rs` and `service/runtime/usage.rs`. | `record_local_upsert` / `record_local_delete_with_type` bump the version vector and set/clear tombstones in the same transaction. Broadcast starts after commit via `maybe_broadcast_local_change`. | This helper family is one local write path; direct SQL in unrelated service handlers still needs classification. |
| Local canonical objects ([objects.rs](../core/crates/ark-core/src/service/runtime/objects.rs#L218), [db/objects/part2a.rs](../core/crates/ark-core/src/db/objects/part2a.rs#L1)) | `upsert_object` / `delete_object` validate optional snapshots and mutate canonical object storage inside `with_write_tx`. | Version vector/tombstone and `object_sync_versions` updates are in that transaction; live broadcast follows success. | Type and link RPC handlers are separate, under `service/runtime/types.rs`; audit them if expanding canonical writes. |
| Registry and pending replay ([runtime/types.rs](../core/crates/ark-core/src/service/runtime/types.rs#L10), [queue.rs](../core/crates/ark-core/src/canonical_types/pending/queue.rs#L9), [db/objects/part1.rs](../core/crates/ark-core/src/db/objects/part1.rs#L12)) | Package registration uses `with_write_tx` around `type_registry::register_type`; pending insertion/replay uses the caller's SQLite transaction/savepoint. | Ordinary adapter object apply queues an unknown type version; replay applies matching pending payloads after registry arrival. | Current replay ownership is shared between registry, canonical pending queue, and legacy DB registration logic. |
| Regular remote apply ([sync_server_inbound.rs](../core/crates/ark-core/src/sync_server_inbound.rs#L54), [backend/apply.rs](../core/crates/ark-core/src/db/backend/apply.rs#L28), [backend/adapter.rs](../core/crates/ark-core/src/db/backend/adapter.rs#L223)) | `StorageBackend::apply_entity` dispatches blocking SQLite work; the apply helper uses a savepoint for each entity. | Entity HLC, tombstone, usage sequence, and canonical object state are applied by the existing DB helpers. | Bulk callers include paged server/client sync and relay (`sync_server_messages.rs`, `sync_client_start/part03.rs`, `relay_impl_2.rs`); they own their cursor policy. |
| Signed remote apply ([sync_server_inbound.rs](../core/crates/ark-core/src/sync_server_inbound.rs#L7), [backend/adapter.rs](../core/crates/ark-core/src/db/backend/adapter.rs#L89)) | Authenticated handlers call the signed adapter method; `apply_signed_integration_changes` performs the transaction. | Envelope identity, origin/recipient, replay reservation, and mutation are checked/applied by signed integration code. | Outbound validation is separate in `integration_replication/outbound.rs`; it is not the ordinary entity apply path. |
| Special local broadcast ingress ([service/sync.rs](../core/crates/ark-core/src/service/sync.rs#L192)) | `handle_broadcast_change` persists with `runtime.storage.apply_entity` before fan-out. | Stamps an HLC through the sync server and persists so future vector exchange observes the entity. | Distinct from local RPC writes, which already committed before broadcast. |
| Bootstrap and maintenance ([db/open.rs](../core/crates/ark-core/src/db/open.rs#L32), [db/load/part1.rs](../core/crates/ark-core/src/db/load/part1.rs#L5), [db/compact.rs](../core/crates/ark-core/src/db/compact.rs#L83), [db/sync/part2.rs](../core/crates/ark-core/src/db/sync/part2.rs#L1)) | Open/init owns schema and migrations; clear/delete-trashed and compaction each have their own DB transaction boundaries. | `delete_trashed` records tombstones; version-vector and usage journal helpers own sequence metadata; compaction retains data according to cursors. | Bootstrap/migration, cache, and snapshot SQL are not all syncable writes and remain outside this census. |

## Behavior tests

- `db/tests_sync/part1.rs`–`part2.rs`: backend SQLite round-trips, invalid payload/type rejection, usage paging, deletes/tombstones, KV, and stored HLC.
- `db/tests_sync/part3.rs` and `part5.rs`: pending replay and unknown-version preservation/replay.
- `db/tests_sync/part4.rs`, `part6.rs`, and `part7.rs`: direct adapter collection/apply, compaction boundaries, cursor behavior, and relay convergence.
- `db/tests_integration_replication_apply.rs` and `integration_replication/signed_sync_tests.rs`: signed authorization and atomic apply.
- `tests/it/sync_round_trip.rs`: regular two-server and selective-profile replication.
- Local object behavior also has coverage in `service/tests/object_write_snapshot.rs`, `object_revision_compat.rs`, and `sync_lifecycle.rs`.

The extraction's SQLite behavior pin was `cargo nextest run -p ark-core storage_backend` (13 tests) before and after moving the modules. The full crate suite passes after the move; exact commands and exit statuses are in `.audit/modules-receipts.md`.

## Reproduce and limits

From the Cortex worktree root, locate the relevant write entry points and call sites with:

```sh
rg -n 'record_local_upsert|record_local_delete_with_type|handle_broadcast_change|upsert_object\(|delete_object\(|types_register_package_definitions|insert_pending_object|replay_pending_for_type|clear_all\(|delete_trashed\(|compact_usage_sync_log|usage_log_complete_through|record_usage_sequence|bump_sync_version_vector' core/crates/ark-core/src/{service,db,canonical_types} -g '*.rs'
rg -n '\.apply_entity\(|apply_signed_integration_frame|handle_live_change|handle_signed_integration_frame|SyncChanges|apply_entities\(' core/crates/ark-core/src/{sync_server*,sync_client*,relay_impl_2.rs,service,integration_replication} -g '*.rs'
```

Known gap: these searches trace current adapter callers and representative local paths but do not classify all `execute` calls. To extend coverage, search direct mutation statements and classify each table/transaction owner:

```sh
rg -n 'execute(_batch)?\(|INSERT INTO|UPDATE [[:alnum:]_]+ SET|DELETE FROM' core/crates/ark-core/src/{db,service,canonical_types} -g '*.rs'
```

KOS-430 separately hardens `with_write_tx` with an immediate rusqlite transaction guard. Regression tests reproduce deferred-COMMIT failure and panic cleanup against the previous helper and pass after the correction. This fixes the existing local helper; local and remote write paths remain separate. Exact verification is recorded in `.audit/transaction-receipts.md`.
