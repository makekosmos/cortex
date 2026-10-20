# ARK Core storage census

Generated from the current Core source. The scanner found 21 typed operations and 846 SQLite ownership references. It scans all Rust sources and labels standalone tests, inline tests, debug-only code, and production separately. It does not infer policy ownership or discover indirect SQL hidden behind helper calls.

## Transaction contracts

| Command contract | Count | Execution boundary |
| --- | ---: | --- |
| `DbCommand` | 18 | immediate write transaction |
| `DbQuery` | 2 | deferred read transaction |
| `DbMaintenanceCommand` | 1 | exclusive maintenance connection |

## Typed operation inventory

| Kind | Operation | Source |
| --- | --- | --- |
| `DbCommand` | `AppendLocalRegistryEventCommand` | [core/crates/ark-core/src/db/command/commands/device_registry_command.rs:26](../core/crates/ark-core/src/db/command/commands/device_registry_command.rs:26) |
| `DbCommand` | `SignRegistryHandshakeCommand` | [core/crates/ark-core/src/db/command/commands/device_registry_handshake.rs:25](../core/crates/ark-core/src/db/command/commands/device_registry_handshake.rs:25) |
| `DbCommand` | `ApplyRegistryEventsCommand` | [core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:149](../core/crates/ark-core/src/db/command/commands/device_registry_sync.rs:149) |
| `DbCommand` | `EnrollPairedDeviceCommand` | [core/crates/ark-core/src/db/command/commands/device_registry.rs:43](../core/crates/ark-core/src/db/command/commands/device_registry.rs:43) |
| `DbCommand` | `ReceiveInboxReceiptCommand` | [core/crates/ark-core/src/db/command/commands/inbound_policy.rs:67](../core/crates/ark-core/src/db/command/commands/inbound_policy.rs:67) |
| `DbCommand` | `PersistNodeAuthorizationCommand` | [core/crates/ark-core/src/db/command/commands/integration_admin.rs:36](../core/crates/ark-core/src/db/command/commands/integration_admin.rs:36) |
| `DbCommand` | `PersistIntegrationGrantCommand` | [core/crates/ark-core/src/db/command/commands/integration_admin.rs:76](../core/crates/ark-core/src/db/command/commands/integration_admin.rs:76) |
| `DbCommand` | `AcquireIntegrationRefreshLeaseCommand` | [core/crates/ark-core/src/db/command/commands/integration_admin.rs:109](../core/crates/ark-core/src/db/command/commands/integration_admin.rs:109) |
| `DbCommand` | `PublishIntegrationCredentialEnvelopeCommand` | [core/crates/ark-core/src/db/command/commands/integration_admin.rs:152](../core/crates/ark-core/src/db/command/commands/integration_admin.rs:152) |
| `DbCommand` | `ApplySignedIntegrationFrameCommand` | [core/crates/ark-core/src/db/command/commands/integration_replication.rs:103](../core/crates/ark-core/src/db/command/commands/integration_replication.rs:103) |
| `DbCommand` | `LegacyDeleteCommand` | [core/crates/ark-core/src/db/command/commands/legacy_mutation/legacy_delete.rs:20](../core/crates/ark-core/src/db/command/commands/legacy_mutation/legacy_delete.rs:20) |
| `DbCommand` | `LegacyGraphWriteCommand` | [core/crates/ark-core/src/db/command/commands/legacy_mutation/legacy_graph.rs:20](../core/crates/ark-core/src/db/command/commands/legacy_mutation/legacy_graph.rs:20) |
| `DbCommand` | `ApplySyncEntityCommand` | [core/crates/ark-core/src/db/command/commands/sync_mutation.rs:131](../core/crates/ark-core/src/db/command/commands/sync_mutation.rs:131) |
| `DbCommand` | `ApplyAuthenticatedSyncEntityCommand` | [core/crates/ark-core/src/db/command/commands/sync_mutation.rs:192](../core/crates/ark-core/src/db/command/commands/sync_mutation.rs:192) |
| `DbCommand` | `PrepareTaskMergeBatchCommand` | [core/crates/ark-core/src/db/command/commands/task_merge.rs:49](../core/crates/ark-core/src/db/command/commands/task_merge.rs:49) |
| `DbCommand` | `AcknowledgeTaskMergeBatchCommand` | [core/crates/ark-core/src/db/command/commands/task_merge.rs:100](../core/crates/ark-core/src/db/command/commands/task_merge.rs:100) |
| `DbCommand` | `ApplyTaskMergeBatchCommand` | [core/crates/ark-core/src/db/command/commands/task_merge.rs:152](../core/crates/ark-core/src/db/command/commands/task_merge.rs:152) |
| `DbCommand` | `OperationReceiptCommand` | [core/crates/ark-core/src/db/command/operation_receipt.rs:47](../core/crates/ark-core/src/db/command/operation_receipt.rs:47) |
| `DbMaintenanceCommand` | `RestoreSnapshotCommand` | [core/crates/ark-core/src/db/command/commands/system_mutation.rs:97](../core/crates/ark-core/src/db/command/commands/system_mutation.rs:97) |
| `DbQuery` | `IntegrationReadQuery` | [core/crates/ark-core/src/db/command/commands/integration_read.rs:59](../core/crates/ark-core/src/db/command/commands/integration_read.rs:59) |
| `DbQuery` | `ValidateOutboundSignedFrameQuery` | [core/crates/ark-core/src/db/command/commands/integration_replication.rs:23](../core/crates/ark-core/src/db/command/commands/integration_replication.rs:23) |

## SQLite references by scope

| Scope | Category | Matches |
| --- | --- | ---: |
| debug_only | SQLite type or import | 5 |
| inline_test | SQL literal | 1 |
| inline_test | SQLite lifecycle | 3 |
| inline_test | SQLite operation | 2 |
| inline_test | SQLite type or import | 4 |
| production | rusqlite dependency | 1 |
| production | SQL literal | 19 |
| production | SQLite lifecycle | 9 |
| production | SQLite operation | 20 |
| production | SQLite type or import | 106 |
| test_file | SQL literal | 290 |
| test_file | SQLite lifecycle | 81 |
| test_file | SQLite operation | 248 |
| test_file | SQLite type or import | 57 |
