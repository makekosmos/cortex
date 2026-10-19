# ARK Core storage census

Generated from the current Core source. The scanner found 70 typed operations and 1271 SQLite ownership references. It scans all Rust sources and labels standalone tests, inline tests, debug-only code, and production separately. It does not infer policy ownership or discover indirect SQL hidden behind helper calls.

## Transaction contracts

| Command contract | Count | Execution boundary |
| --- | ---: | --- |
| `DbCommand` | 45 | immediate write transaction |
| `DbQuery` | 24 | deferred read transaction |
| `DbMaintenanceCommand` | 1 | exclusive maintenance connection |

## Typed operation inventory

| Kind | Operation | Source |
| --- | --- | --- |
| `DbCommand` | `LocalCoreTaskWrite` | [core/crates/ark-core/src/db/command/commands/core_task.rs:32](../core/crates/ark-core/src/db/command/commands/core_task.rs:32) |
| `DbCommand` | `AuthenticatedCoreTaskWrite` | [core/crates/ark-core/src/db/command/commands/core_task.rs:131](../core/crates/ark-core/src/db/command/commands/core_task.rs:131) |
| `DbCommand` | `AppendLocalRegistryEventCommand` | [core/crates/ark-core/src/db/command/commands/device_registry_command.rs:26](../core/crates/ark-core/src/db/command/commands/device_registry_command.rs:26) |
| `DbCommand` | `SignRegistryHandshakeCommand` | [core/crates/ark-core/src/db/command/commands/device_registry_handshake.rs:25](../core/crates/ark-core/src/db/command/commands/device_registry_handshake.rs:25) |
| `DbCommand` | `ApplyRegistryEventsCommand` | [core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:150](../core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:150) |
| `DbCommand` | `EnrollPairedDeviceCommand` | [core/crates/ark-core/src/db/command/commands/device_registry.rs:44](../core/crates/ark-core/src/db/command/commands/device_registry.rs:44) |
| `DbCommand` | `UpsertExternalRefCommand` | [core/crates/ark-core/src/db/command/commands/external_ref.rs:19](../core/crates/ark-core/src/db/command/commands/external_ref.rs:19) |
| `DbCommand` | `ReceiveInboxReceiptCommand` | [core/crates/ark-core/src/db/command/commands/inbound_policy.rs:67](../core/crates/ark-core/src/db/command/commands/inbound_policy.rs:67) |
| `DbCommand` | `PersistNodeAuthorizationCommand` | [core/crates/ark-core/src/db/command/commands/integration_admin.rs:36](../core/crates/ark-core/src/db/command/commands/integration_admin.rs:36) |
| `DbCommand` | `PersistIntegrationGrantCommand` | [core/crates/ark-core/src/db/command/commands/integration_admin.rs:76](../core/crates/ark-core/src/db/command/commands/integration_admin.rs:76) |
| `DbCommand` | `AcquireIntegrationRefreshLeaseCommand` | [core/crates/ark-core/src/db/command/commands/integration_admin.rs:109](../core/crates/ark-core/src/db/command/commands/integration_admin.rs:109) |
| `DbCommand` | `PublishIntegrationCredentialEnvelopeCommand` | [core/crates/ark-core/src/db/command/commands/integration_admin.rs:152](../core/crates/ark-core/src/db/command/commands/integration_admin.rs:152) |
| `DbCommand` | `ApplySignedIntegrationFrameCommand` | [core/crates/ark-core/src/db/command/commands/integration_replication.rs:103](../core/crates/ark-core/src/db/command/commands/integration_replication.rs:103) |
| `DbCommand` | `LegacyDeleteCommand` | [core/crates/ark-core/src/db/command/commands/legacy_mutation/legacy_delete.rs:20](../core/crates/ark-core/src/db/command/commands/legacy_mutation/legacy_delete.rs:20) |
| `DbCommand` | `LegacyGraphWriteCommand` | [core/crates/ark-core/src/db/command/commands/legacy_mutation/legacy_graph.rs:20](../core/crates/ark-core/src/db/command/commands/legacy_mutation/legacy_graph.rs:20) |
| `DbCommand` | `TrackedAppUpsertCommand` | [core/crates/ark-core/src/db/command/commands/legacy_mutation/tracked_app.rs:22](../core/crates/ark-core/src/db/command/commands/legacy_mutation/tracked_app.rs:22) |
| `DbCommand` | `TrackedAppDeleteCommand` | [core/crates/ark-core/src/db/command/commands/legacy_mutation/tracked_app.rs:55](../core/crates/ark-core/src/db/command/commands/legacy_mutation/tracked_app.rs:55) |
| `DbCommand` | `LocalBookCoverCommand` | [core/crates/ark-core/src/db/command/commands/object_mutation/book_cover.rs:20](../core/crates/ark-core/src/db/command/commands/object_mutation/book_cover.rs:20) |
| `DbCommand` | `CanonicalGameUpsertCommand` | [core/crates/ark-core/src/db/command/commands/object_mutation/canonical_game.rs:25](../core/crates/ark-core/src/db/command/commands/object_mutation/canonical_game.rs:25) |
| `DbCommand` | `LocalObjectDeleteCommand` | [core/crates/ark-core/src/db/command/commands/object_mutation/delete.rs:22](../core/crates/ark-core/src/db/command/commands/object_mutation/delete.rs:22) |
| `DbCommand` | `LocalObjectUpsertCommand` | [core/crates/ark-core/src/db/command/commands/object_mutation/upsert.rs:22](../core/crates/ark-core/src/db/command/commands/object_mutation/upsert.rs:22) |
| `DbCommand` | `CommittedFeedProgressCommand` | [core/crates/ark-core/src/db/command/commands/sync_mutation.rs:70](../core/crates/ark-core/src/db/command/commands/sync_mutation.rs:70) |
| `DbCommand` | `EnsureUsageSequenceMigratedCommand` | [core/crates/ark-core/src/db/command/commands/sync_mutation.rs:230](../core/crates/ark-core/src/db/command/commands/sync_mutation.rs:230) |
| `DbCommand` | `ApplySyncEntityCommand` | [core/crates/ark-core/src/db/command/commands/sync_mutation.rs:263](../core/crates/ark-core/src/db/command/commands/sync_mutation.rs:263) |
| `DbCommand` | `ApplyAuthenticatedSyncEntityCommand` | [core/crates/ark-core/src/db/command/commands/sync_mutation.rs:324](../core/crates/ark-core/src/db/command/commands/sync_mutation.rs:324) |
| `DbCommand` | `SetSyncKeyValueCommand` | [core/crates/ark-core/src/db/command/commands/sync_mutation.rs:428](../core/crates/ark-core/src/db/command/commands/sync_mutation.rs:428) |
| `DbCommand` | `SetLocalSyncKeyValueCommand` | [core/crates/ark-core/src/db/command/commands/sync_mutation.rs:467](../core/crates/ark-core/src/db/command/commands/sync_mutation.rs:467) |
| `DbCommand` | `SaveLocalSyncProfileCommand` | [core/crates/ark-core/src/db/command/commands/sync_policy.rs:36](../core/crates/ark-core/src/db/command/commands/sync_policy.rs:36) |
| `DbCommand` | `AcceptPeerSyncPolicyCommand` | [core/crates/ark-core/src/db/command/commands/sync_policy.rs:149](../core/crates/ark-core/src/db/command/commands/sync_policy.rs:149) |
| `DbCommand` | `ClearAllCommand` | [core/crates/ark-core/src/db/command/commands/system_mutation.rs:10](../core/crates/ark-core/src/db/command/commands/system_mutation.rs:10) |
| `DbCommand` | `DeleteTrashedCommand` | [core/crates/ark-core/src/db/command/commands/system_mutation.rs:41](../core/crates/ark-core/src/db/command/commands/system_mutation.rs:41) |
| `DbCommand` | `LoadOrGenerateIrohSecretKeyCommand` | [core/crates/ark-core/src/db/command/commands/system_mutation.rs:64](../core/crates/ark-core/src/db/command/commands/system_mutation.rs:64) |
| `DbCommand` | `CompactUsageSyncLogCommand` | [core/crates/ark-core/src/db/command/commands/system_mutation.rs:106](../core/crates/ark-core/src/db/command/commands/system_mutation.rs:106) |
| `DbCommand` | `AppendTaskMergeOperationCommand` | [core/crates/ark-core/src/db/command/commands/task_merge.rs:31](../core/crates/ark-core/src/db/command/commands/task_merge.rs:31) |
| `DbCommand` | `PrepareTaskMergeBatchCommand` | [core/crates/ark-core/src/db/command/commands/task_merge.rs:74](../core/crates/ark-core/src/db/command/commands/task_merge.rs:74) |
| `DbCommand` | `AcknowledgeTaskMergeBatchCommand` | [core/crates/ark-core/src/db/command/commands/task_merge.rs:125](../core/crates/ark-core/src/db/command/commands/task_merge.rs:125) |
| `DbCommand` | `ApplyTaskMergeBatchCommand` | [core/crates/ark-core/src/db/command/commands/task_merge.rs:177](../core/crates/ark-core/src/db/command/commands/task_merge.rs:177) |
| `DbCommand` | `RegisterPackageTypesCommand` | [core/crates/ark-core/src/db/command/commands/type_mutation.rs:27](../core/crates/ark-core/src/db/command/commands/type_mutation.rs:27) |
| `DbCommand` | `UpsertObjectTypeCommand` | [core/crates/ark-core/src/db/command/commands/type_mutation.rs:65](../core/crates/ark-core/src/db/command/commands/type_mutation.rs:65) |
| `DbCommand` | `DeleteObjectTypeCommand` | [core/crates/ark-core/src/db/command/commands/type_mutation.rs:100](../core/crates/ark-core/src/db/command/commands/type_mutation.rs:100) |
| `DbCommand` | `UpsertObjectLinkCommand` | [core/crates/ark-core/src/db/command/commands/type_mutation.rs:138](../core/crates/ark-core/src/db/command/commands/type_mutation.rs:138) |
| `DbCommand` | `DeleteObjectLinkCommand` | [core/crates/ark-core/src/db/command/commands/type_mutation.rs:173](../core/crates/ark-core/src/db/command/commands/type_mutation.rs:173) |
| `DbCommand` | `ReplaceObjectLinkCommand` | [core/crates/ark-core/src/db/command/commands/type_mutation.rs:211](../core/crates/ark-core/src/db/command/commands/type_mutation.rs:211) |
| `DbCommand` | `LocalUsageWriteCommand` | [core/crates/ark-core/src/db/command/commands/usage.rs:21](../core/crates/ark-core/src/db/command/commands/usage.rs:21) |
| `DbCommand` | `OperationReceiptCommand` | [core/crates/ark-core/src/db/command/operation_receipt.rs:40](../core/crates/ark-core/src/db/command/operation_receipt.rs:40) |
| `DbMaintenanceCommand` | `RestoreSnapshotCommand` | [core/crates/ark-core/src/db/command/commands/system_mutation.rs:145](../core/crates/ark-core/src/db/command/commands/system_mutation.rs:145) |
| `DbQuery` | `RegistryPeerAdmissionQuery` | [core/crates/ark-core/src/db/command/commands/device_registry_admission.rs:18](../core/crates/ark-core/src/db/command/commands/device_registry_admission.rs:18) |
| `DbQuery` | `RegistryDeviceEnabledQuery` | [core/crates/ark-core/src/db/command/commands/device_registry_admission.rs:65](../core/crates/ark-core/src/db/command/commands/device_registry_admission.rs:65) |
| `DbQuery` | `VerifyRegistryHandshakeQuery` | [core/crates/ark-core/src/db/command/commands/device_registry_handshake.rs:84](../core/crates/ark-core/src/db/command/commands/device_registry_handshake.rs:84) |
| `DbQuery` | `LoadDeviceRegistryQuery` | [core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:27](../core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:27) |
| `DbQuery` | `LoadRegistryEventsQuery` | [core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:94](../core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:94) |
| `DbQuery` | `IntegrationReadQuery` | [core/crates/ark-core/src/db/command/commands/integration_read.rs:59](../core/crates/ark-core/src/db/command/commands/integration_read.rs:59) |
| `DbQuery` | `ValidateOutboundSignedFrameQuery` | [core/crates/ark-core/src/db/command/commands/integration_replication.rs:23](../core/crates/ark-core/src/db/command/commands/integration_replication.rs:23) |
| `DbQuery` | `ObjectReadQuery` | [core/crates/ark-core/src/db/command/commands/object_read.rs:29](../core/crates/ark-core/src/db/command/commands/object_read.rs:29) |
| `DbQuery` | `LoadActiveLocalSyncProfileQuery` | [core/crates/ark-core/src/db/command/commands/sync_policy.rs:88](../core/crates/ark-core/src/db/command/commands/sync_policy.rs:88) |
| `DbQuery` | `LoadAcceptedPeerSyncPolicyQuery` | [core/crates/ark-core/src/db/command/commands/sync_policy.rs:198](../core/crates/ark-core/src/db/command/commands/sync_policy.rs:198) |
| `DbQuery` | `LoadSyncProfileForSyncQuery` | [core/crates/ark-core/src/db/command/commands/sync_policy.rs:232](../core/crates/ark-core/src/db/command/commands/sync_policy.rs:232) |
| `DbQuery` | `LoadPolicyBoundEntitiesPageQuery` | [core/crates/ark-core/src/db/command/commands/sync_read_policy.rs:40](../core/crates/ark-core/src/db/command/commands/sync_read_policy.rs:40) |
| `DbQuery` | `PolicyBoundFeedDeliveryPageQuery` | [core/crates/ark-core/src/db/command/commands/sync_read_policy.rs:126](../core/crates/ark-core/src/db/command/commands/sync_read_policy.rs:126) |
| `DbQuery` | `LoadEntitiesQuery` | [core/crates/ark-core/src/db/command/commands/sync_read.rs:81](../core/crates/ark-core/src/db/command/commands/sync_read.rs:81) |
| `DbQuery` | `UsageCompleteThroughQuery` | [core/crates/ark-core/src/db/command/commands/sync_read.rs:188](../core/crates/ark-core/src/db/command/commands/sync_read.rs:188) |
| `DbQuery` | `CommittedFeedSnapshotQuery` | [core/crates/ark-core/src/db/command/commands/sync_read.rs:220](../core/crates/ark-core/src/db/command/commands/sync_read.rs:220) |
| `DbQuery` | `CommittedFeedPageQuery` | [core/crates/ark-core/src/db/command/commands/sync_read.rs:260](../core/crates/ark-core/src/db/command/commands/sync_read.rs:260) |
| `DbQuery` | `CommittedFeedDeliveryPageQuery` | [core/crates/ark-core/src/db/command/commands/sync_read.rs:306](../core/crates/ark-core/src/db/command/commands/sync_read.rs:306) |
| `DbQuery` | `GetSyncKeyValueQuery` | [core/crates/ark-core/src/db/command/commands/sync_read.rs:347](../core/crates/ark-core/src/db/command/commands/sync_read.rs:347) |
| `DbQuery` | `AuthorizedTransportKeyQuery` | [core/crates/ark-core/src/db/command/commands/sync_read.rs:381](../core/crates/ark-core/src/db/command/commands/sync_read.rs:381) |
| `DbQuery` | `SystemReadQuery` | [core/crates/ark-core/src/db/command/commands/system_read.rs:11](../core/crates/ark-core/src/db/command/commands/system_read.rs:11) |
| `DbQuery` | `TaskConflictSetQuery` | [core/crates/ark-core/src/db/command/commands/task_merge.rs:258](../core/crates/ark-core/src/db/command/commands/task_merge.rs:258) |
| `DbQuery` | `TypeReadQuery` | [core/crates/ark-core/src/db/command/commands/type_read.rs:24](../core/crates/ark-core/src/db/command/commands/type_read.rs:24) |
| `DbQuery` | `UsageReadQuery` | [core/crates/ark-core/src/db/command/commands/usage_read.rs:31](../core/crates/ark-core/src/db/command/commands/usage_read.rs:31) |

## SQLite references by scope

| Scope | Category | Matches |
| --- | --- | ---: |
| debug_only | SQLite type or import | 2 |
| inline_test | SQL literal | 1 |
| inline_test | SQLite lifecycle | 4 |
| inline_test | SQLite operation | 2 |
| inline_test | SQLite type or import | 4 |
| production | rusqlite dependency | 1 |
| production | SQL literal | 216 |
| production | SQLite lifecycle | 12 |
| production | SQLite operation | 206 |
| production | SQLite type or import | 188 |
| test_file | SQL literal | 275 |
| test_file | SQLite lifecycle | 76 |
| test_file | SQLite operation | 236 |
| test_file | SQLite type or import | 48 |
