# ARK typed-operation transaction map

This map groups the 55 operations still found by the generated [Core storage census](ark-core-storage-census-2026-10-19.md), plus four extracted Store writer paths, by responsibility. The current inventory has **30 `DbCommand` writes, 24 `DbQuery` reads, and 1 `DbMaintenanceCommand`**; local and authenticated `core.task` writes, local sync key-value writes, and sync-progress writes use dedicated Store writer tasks. `OperationReceiptCommand` remains in the census as a test-only adapter. Locations point to the current production owners.

## Shared execution and effects

Remaining `DbCommand` operations execute inside the writer's immediate transaction (`core/crates/ark-core/src/db/command/executor/actor/task.rs:48-61`). `DbQuery` runs against a reader snapshot in a deferred read transaction (`core/crates/ark-core/src/db/command/handle.rs:177-195`). The maintenance command is admitted through the exclusive maintenance path and operates on a mutable connection (`core/crates/ark-core/src/db/command/handle.rs:130-161`). `LocalCoreTaskWrite` now uses a dedicated actor task and Store-owned writer method. No command-specific savepoint was found in the typed implementations covered here.

These operations return results to their Core callers; no direct network or other post-commit effect was found in their `execute`/`read` bodies. A caller may use a returned result to initiate later work, but caller-side follow-up was not traced here. The generic `OperationReceiptCommand` is test-only; the migrated task writer now persists its operation receipt and task state atomically in Store.

## Responsibility groups

### Task writes and inbox policy — 3 operations

**Core:** owns authenticated transport context checks and actor scheduling. Application owns the authenticated task use case, sync-policy identity verification, inbox/stale outcomes, and task representation decoding. Store owns the SQLite-backed policy/admission/inbox/object/task sessions and the immediate transaction. The local task path also uses Store-owned sessions, readback, and operation receipts.

| Kind | Operation | Implementation |
|---|---|---|
| StoreWriterTask | `LocalCoreTaskWrite` | `core/crates/ark-core/src/db/command/executor/actor/task/core_task_write.rs` → `ark_store::writer::SqliteWriter::apply_local_core_task_write` |
| StoreWriterTask | `AuthenticatedCoreTaskWrite` | `core/crates/ark-core/src/db/command/executor/actor/task/authenticated_core_task_write.rs` → `ark_store::writer::SqliteWriter::apply_authenticated_core_task_write` |
| DbCommand | `ReceiveInboxReceiptCommand` | `core/crates/ark-core/src/db/command/commands/inbound_policy.rs:67` |

### Device registry — 9 operations

**Core:** owns actor identity, event and handshake validation, admission policy, and registry synchronization decisions. **Persistence:** Store `device_registry` and `sync_kv` functions load or persist registry entries, events, projections, and transport-key pins. Writes use the immediate transaction; queries use the deferred snapshot.

| Kind | Operation | Implementation |
|---|---|---|
| DbCommand | `AppendLocalRegistryEventCommand` | `core/crates/ark-core/src/db/command/commands/device_registry_command.rs:26` |
| DbCommand | `SignRegistryHandshakeCommand` | `core/crates/ark-core/src/db/command/commands/device_registry_handshake.rs:25` |
| DbCommand | `ApplyRegistryEventsCommand` | `core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:150` |
| DbCommand | `EnrollPairedDeviceCommand` | `core/crates/ark-core/src/db/command/commands/device_registry.rs:43` |
| DbQuery | `RegistryPeerAdmissionQuery` | `core/crates/ark-core/src/db/command/commands/device_registry_admission.rs:18` |
| DbQuery | `RegistryDeviceEnabledQuery` | `core/crates/ark-core/src/db/command/commands/device_registry_admission.rs:65` |
| DbQuery | `VerifyRegistryHandshakeQuery` | `core/crates/ark-core/src/db/command/commands/device_registry_handshake.rs:84` |
| DbQuery | `LoadDeviceRegistryQuery` | `core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:27` |
| DbQuery | `LoadRegistryEventsQuery` | `core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:94` |

### Integration replication and administration — 7 operations

**Core:** owns principal checks, signed-frame validation, issuer and credential-fence policy, and command orchestration. **Persistence:** split between `ark_store::integration_replication` record APIs and Core `crate::db` transaction helpers/sessions. Writes use the immediate transaction; reads use the deferred snapshot. Refresh and publication operations only persist local lease/envelope state here; network behavior is not part of these typed operations.

| Kind | Operation | Implementation |
|---|---|---|
| DbCommand | `PersistNodeAuthorizationCommand` | `core/crates/ark-core/src/db/command/commands/integration_admin.rs:36` |
| DbCommand | `PersistIntegrationGrantCommand` | `core/crates/ark-core/src/db/command/commands/integration_admin.rs:76` |
| DbCommand | `AcquireIntegrationRefreshLeaseCommand` | `core/crates/ark-core/src/db/command/commands/integration_admin.rs:109` |
| DbCommand | `PublishIntegrationCredentialEnvelopeCommand` | `core/crates/ark-core/src/db/command/commands/integration_admin.rs:152` |
| DbCommand | `ApplySignedIntegrationFrameCommand` | `core/crates/ark-core/src/db/command/commands/integration_replication.rs:103` |
| DbQuery | `ValidateOutboundSignedFrameQuery` | `core/crates/ark-core/src/db/command/commands/integration_replication.rs:23` |
| DbQuery | `IntegrationReadQuery` | `core/crates/ark-core/src/db/command/commands/integration_read.rs:59` |

### Legacy mutations — 4 operations

**Core:** command wrappers use Core `SqliteLegacyMutationSession`. **Persistence:** Store has legacy-entity mutation helpers, but the session implementation and indirect calls must be traced before assigning complete Store ownership. Writes use the immediate transaction. This is an ownership gap, not evidence that all mutation SQL is in Core.

| Kind | Operation | Implementation |
|---|---|---|
| DbCommand | `LegacyDeleteCommand` | `core/crates/ark-core/src/db/command/commands/legacy_mutation/legacy_delete.rs:20` |
| DbCommand | `LegacyGraphWriteCommand` | `core/crates/ark-core/src/db/command/commands/legacy_mutation/legacy_graph.rs:20` |
| DbCommand | `TrackedAppUpsertCommand` | `core/crates/ark-core/src/db/command/commands/legacy_mutation/tracked_app.rs:22` |
| DbCommand | `TrackedAppDeleteCommand` | `core/crates/ark-core/src/db/command/commands/legacy_mutation/tracked_app.rs:55` |

### Sync policy, entity/feed transfer, KV, and usage log — 20 operations

**Core:** owns principal and class checks, peer authorization, profile/policy binding, sync visibility decisions, authenticated inbox handling, and feed-size validation. **Persistence:** Store `data_platform`, `sync_entities`, `committed_feed`, `sync_kv`, `usage_sync_log`, and authorized-node functions provide the storage operations. Writes use the immediate transaction; reads use one deferred snapshot. The authenticated apply path keeps inbox and entity changes atomic.

| Kind | Operation | Implementation |
|---|---|---|
| DbCommand | `CommittedFeedProgressCommand` | `core/crates/ark-core/src/db/command/commands/sync_mutation.rs:70` |
| DbCommand | `EnsureUsageSequenceMigratedCommand` | `core/crates/ark-core/src/db/command/commands/sync_mutation.rs:230` |
| DbCommand | `ApplySyncEntityCommand` | `core/crates/ark-core/src/db/command/commands/sync_mutation.rs:263` |
| DbCommand | `ApplyAuthenticatedSyncEntityCommand` | `core/crates/ark-core/src/db/command/commands/sync_mutation.rs:324` |
| DbCommand | `SaveLocalSyncProfileCommand` | `core/crates/ark-core/src/db/command/commands/sync_policy.rs:36` |
| DbCommand | `AcceptPeerSyncPolicyCommand` | `core/crates/ark-core/src/db/command/commands/sync_policy.rs:149` |
| DbQuery | `LoadActiveLocalSyncProfileQuery` | `core/crates/ark-core/src/db/command/commands/sync_policy.rs:88` |
| DbQuery | `LoadAcceptedPeerSyncPolicyQuery` | `core/crates/ark-core/src/db/command/commands/sync_policy.rs:198` |
| DbQuery | `LoadSyncProfileForSyncQuery` | `core/crates/ark-core/src/db/command/commands/sync_policy.rs:232` |
| DbQuery | `LoadPolicyBoundEntitiesPageQuery` | `core/crates/ark-core/src/db/command/commands/sync_read_policy.rs:42` |
| DbQuery | `PolicyBoundFeedDeliveryPageQuery` | `core/crates/ark-core/src/db/command/commands/sync_read_policy.rs:113` |
| DbQuery | `LoadEntitiesQuery` | `core/crates/ark-core/src/db/command/commands/sync_read.rs:76` |
| DbQuery | `UsageCompleteThroughQuery` | `core/crates/ark-core/src/db/command/commands/sync_read.rs:180` |
| DbQuery | `CommittedFeedSnapshotQuery` | `core/crates/ark-core/src/db/command/commands/sync_read.rs:212` |
| DbQuery | `CommittedFeedPageQuery` | `core/crates/ark-core/src/db/command/commands/sync_read.rs:252` |
| DbQuery | `CommittedFeedDeliveryPageQuery` | `core/crates/ark-core/src/db/command/commands/sync_read.rs:298` |
| DbQuery | `GetSyncKeyValueQuery` | `core/crates/ark-core/src/db/command/commands/sync_read.rs:339` |
| DbQuery | `AuthorizedTransportKeyQuery` | `core/crates/ark-core/src/db/command/commands/sync_read.rs:373` |

### Task merge — 5 operations

**Core:** applies delivery policy, removed-peer checks, and local sync-profile policy. **Persistence:** task-merge journal, progress, inbound apply, and conflict-read mechanics are in Store, reached through some Core `crate::db::task_merge` adapters. Writes use the immediate transaction; the conflict query uses the deferred snapshot.

| Kind | Operation | Implementation |
|---|---|---|
| DbCommand | `AppendTaskMergeOperationCommand` | `core/crates/ark-core/src/db/command/commands/task_merge.rs:31` |
| DbCommand | `PrepareTaskMergeBatchCommand` | `core/crates/ark-core/src/db/command/commands/task_merge.rs:74` |
| DbCommand | `AcknowledgeTaskMergeBatchCommand` | `core/crates/ark-core/src/db/command/commands/task_merge.rs:125` |
| DbCommand | `ApplyTaskMergeBatchCommand` | `core/crates/ark-core/src/db/command/commands/task_merge.rs:177` |
| DbQuery | `TaskConflictSetQuery` | `core/crates/ark-core/src/db/command/commands/task_merge.rs:258` |

### Type registry — 4 operations

**Core:** mutations use Core `SqliteTypeMutationSession`, so full Store ownership is not established. **Persistence:** type reads call Store `object_types` and `objects` APIs. Writes use the immediate transaction; reads use the deferred snapshot.

| Kind | Operation | Implementation |
|---|---|---|
| DbCommand | `RegisterPackageTypesCommand` | `core/crates/ark-core/src/db/command/commands/type_mutation.rs:25` |
| DbCommand | `UpsertObjectTypeCommand` | `core/crates/ark-core/src/db/command/commands/type_mutation.rs:63` |
| DbCommand | `DeleteObjectTypeCommand` | `core/crates/ark-core/src/db/command/commands/type_mutation.rs:98` |
| DbQuery | `TypeReadQuery` | `core/crates/ark-core/src/db/command/commands/type_read.rs:24` |

### System, object, and usage reads — 6 operations

**Core:** checks engine access and dispatches system/restore behavior. **Persistence:** object and usage reads call Store APIs, while system key-value reads call Store `sync_kv`; snapshot validation and restore remain Core DB orchestration. `SystemReadQuery`, `ObjectReadQuery`, and `UsageReadQuery` use deferred read snapshots. `LoadOrGenerateIrohSecretKeyCommand` and `CompactUsageSyncLogCommand` use immediate transactions. `RestoreSnapshotCommand` is the sole maintenance operation and uses an exclusive maintenance connection. The command-level trace does not establish additional post-commit key or restore effects.

| Kind | Operation | Implementation |
|---|---|---|
| DbCommand | `LoadOrGenerateIrohSecretKeyCommand` | `core/crates/ark-core/src/db/command/commands/system_mutation.rs:10` |
| DbCommand | `CompactUsageSyncLogCommand` | `core/crates/ark-core/src/db/command/commands/system_mutation.rs:52` |
| DbMaintenanceCommand | `RestoreSnapshotCommand` | `core/crates/ark-core/src/db/command/commands/system_mutation.rs:91` |
| DbQuery | `SystemReadQuery` | `core/crates/ark-core/src/db/command/commands/system_read.rs:11` |
| DbQuery | `ObjectReadQuery` | `core/crates/ark-core/src/db/command/commands/object_read.rs:28` |
| DbQuery | `UsageReadQuery` | `core/crates/ark-core/src/db/command/commands/usage_read.rs:31` |

### Operation receipts — 1 operation

**Core:** derives the operation principal, checks command/payload identity, and wraps the inner command. **Persistence:** Store hashes, loads, and inserts the receipt. Inner command and receipt share the immediate transaction. No post-commit effect is visible in the wrapper.

| Kind | Operation | Implementation |
|---|---|---|
| DbCommand | `OperationReceiptCommand` | `core/crates/ark-core/src/db/command/operation_receipt.rs:39` |

## Count reconciliation

| Responsibility group | StoreWriterTask | DbCommand | DbQuery | Maintenance | Total |
|---|---:|---:|---:|---:|---:|
| Task writes and inbox policy | 2 | 1 | 0 | 0 | 3 |
| Device registry | 0 | 4 | 5 | 0 | 9 |
| Integration replication and administration | 0 | 5 | 2 | 0 | 7 |
| Legacy mutations | 0 | 4 | 0 | 0 | 4 |
| Sync policy, transfer, KV, usage log | 0 | 8 | 12 | 0 | 20 |
| Task merge | 0 | 4 | 1 | 0 | 5 |
| Type registry | 0 | 3 | 1 | 0 | 4 |
| System, object, and usage reads | 0 | 2 | 3 | 1 | 6 |
| Operation receipts | 0 | 1 | 0 | 0 | 1 |
| **Total** | **2** | **32** | **24** | **1** | **59** |

The census detects typed operation declarations and direct SQLite references, but does not resolve indirect helper calls or assign policy ownership. Ownership gaps above should be settled by tracing the named Core session/helper implementations and their Store calls before planning those areas as Store-only cutovers.
