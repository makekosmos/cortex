# ARK Core operation ownership map

This map records the current ownership and transaction contract for the typed database operations in `ark-core`, plus startup and restore paths that do not implement `DbCommand`, `DbQuery`, or `DbMaintenanceCommand`. Regenerate the operation inventory with `node scripts/audit-ark-core-storage-census.mjs`. The source inventory is in [`ark-core-storage-census-2026-10-19.md`](ark-core-storage-census-2026-10-19.md).

The map is based on the current command implementations, their adapters, Store SQL helpers, and runtime callers. It is a cutover contract, not a claim that the final Store API already exists. Core still passes raw SQLite connections and transactions into command and adapter code.

## Execution contracts

| Path | Current boundary | Contract to preserve |
| --- | --- | --- |
| `DbCommand` | Actor runs each command in one immediate writer transaction. | Keep validation and all coupled rows in one transaction. Publish results and side effects only after commit. |
| `DbQuery` | Actor takes read admission and the reader barrier, then runs the query on a read-only connection. | Keep a consistent read for compound snapshots. Keep the admission and maintenance ordering in Core. |
| `DbMaintenanceCommand` | Actor takes the exclusive reader barrier and gives the command a mutable connection. | Preserve restore exclusivity, refusal and recovery error classes, and fail-closed behavior. |
| Startup | Actor recovers an interrupted restore, opens the database, initializes schema and rewrites legacy tasks before readiness. | Keep phase order, backups, savepoints, migration checkpoints, and the no-command-before-ready invariant. |

## Operation families

| Family and operations | Policy that stays above Store | Store capability and atomic state | Post-commit behavior |
| --- | --- | --- | --- |
| Object writes: local upsert, delete, book-cover mutation, canonical-game upsert | Engine authorization, caller identity, expected-snapshot checks, canonical type rules, and task-versus-object dispatch. | Resolve and prepare objects; persist objects and links; update revisions, object sync versions, tombstones, and operation receipts. A delete of `core.task` uses task-merge deletion. | Object writes broadcast sync entities and emit object events. Cover and game events fire only when a mutation changed data. |
| Core task writes: local and authenticated | Restrict writes to the supported task type and version. Preserve device identity, snapshot checks, policy identity, registry admission, removed-peer checks, receipt replay, and stale-as-no-op behavior. | Persist task rows, links, task operations, receipts, inbox state, and revisions in the same command transaction. | Broadcast and object events happen after a successful non-replay commit. |
| Task merge: append, prepare batch, acknowledge, apply batch, conflict-set read | Keep local conflict-digest rules, peer matching, accepted-policy identity, full-type task policy, registry admission, removed-peer checks, and batch identity, cursor, position, count, and byte validation. | Keep the operation journal, resolution guards, bootstrap records, task and link projection, revisions, tombstones, in-flight claims, outbound progress, and inbound receipts. Each write remains atomic. | Local non-replay append broadcasts the task-merge batch. Sync adapters consume prepared and acknowledged batches after commit. |
| Object reads: 14 `ObjectReadQuery` variants | Keep query authorization, data-volume scope meaning, and snapshot token selection. | Store owns row selection, decoding, object summaries, searches, canonical projections, and one-snapshot `WriteSnapshot` reads. | Read-only. |
| Type and link writes: package registration, type upsert/delete, link upsert/delete/replace | Keep package/type validation, alias and version rules, and mutation conflict behavior. | Store owns type/link SQL and revision/tombstone updates. Link replacement deletes conflicting links and writes the new link in one transaction. | Return replay notices and sync entities; runtime publishes them after commit. |
| Type reads: summaries, get, versions, alias resolution, object types, object links | Keep alias-resolution behavior and query meaning. | Store owns row selection and result mapping. | Read-only. |
| Legacy writes: graph write, legacy delete, tracked-app upsert/delete | Keep conversion from public records, supported legacy kinds, and caller authorization. | Store owns compatibility persistence. Graph writes can batch records. Tracked-app rows and their sync revisions/tombstones remain atomic. | Runtime broadcasts returned sync entities after commit. |
| Sync writes and reads: entity apply, feed progress, sequence migration, sync KV, policy profiles, entity/feed pages, completion, transport keys | Keep authenticated peer binding, policy identity checks, registry admission, stale-policy outcomes, delivery byte caps, and profile filtering. | Store owns entity and inbox persistence, sync versions, feed cursors and receipts, KV, policy rows, and consistent policy-bound pages. Entity, version, and inbox changes stay atomic. | Sync clients and servers send returned pages and progress after commit. No external network work runs in a Store transaction. |
| Device registry: enrollment, event append/apply, replay and projection reads, handshake signing/verification, admission | Keep self-enrollment rejection, identity parsing, actor authorization, sequence/head rules, signatures, hash-chain conflict and replay rules, admission decisions, and peer binding. | Store owns event logs, projections, actor heads, transport-key pins, and page snapshots. Event/projection/pin updates remain atomic. | Device rename/disable events fan out to server, clients, and relay after commit. |
| Integration: node authorization, grants, leases, credential envelopes, reads, outbound validation, signed inbound frames | Keep Engine or SystemSync context, runtime-space checks, origin/recipient checks, generation/fencing rules, signature and grant validation, transport-key binding, and authenticated sender checks. | Store owns grants, authorization records, leases, envelopes, replication payload reads, replay reservations, entity writes, and version-vector updates. Signed-frame replay reservation and apply remain atomic. | Outbound sends start after validation and commit. No separate post-commit event was found for admin writes. |
| Usage: local session/event/span writes, sequence migration, compaction, and five reads | Keep Engine authorization, usage input validation, runtime defaults, time windows, retention cutoff, and limit clamping. | Store owns usage rows, sequence journal, sync revisions, compaction, analytics, process search, and playtime aggregates. A usage write and its revision rows remain atomic. | Broadcast usage sync entities after commit. Compaction runs under the exclusive maintenance barrier. |
| System: clear, trash deletion, Iroh key load/create, sync-log compaction, snapshot validation | Keep Engine authorization and runtime policy. | Store owns clear/delete operations, atomic key persistence, compaction, and snapshot reads. | No network operation runs within the transaction. |
| Restore | Keep request authorization, sync stop/restart choreography, and conversion to user-facing error classes. | Store owns integrity/schema validation, committed-feed watermark refusal, identity preservation, restore journal, backup application, verification, and rollback. | Restart sync only after the restore outcome is known; recovery-required leaves the actor closed. |
| Canonical startup migration | Keep pure planning, canonical validation, blocker reporting, and startup sequencing. | Store must own migration ledgers, registry/object migration rows, checkpoints, archive and retirement SQL, and transactional failure handling. Pending-object v3 migration already lives in Store. | Actor becomes ready only after recovery, schema initialization, canonical migration, and legacy rewrite succeed. |

## Typed operation coverage

The following inventory groups every typed operation by its current family. Operation names come from the source-aware census; the family contracts above describe the policy, transaction coupling, and post-commit behavior for each group.

| Family | `DbCommand` operations | `DbQuery` operations |
| --- | --- | --- |
| Core task | `LocalCoreTaskWrite`, `AuthenticatedCoreTaskWrite` | — |
| Device registry | `EnrollPairedDeviceCommand`, `AppendLocalRegistryEventCommand`, `ApplyRegistryEventsCommand`, `SignRegistryHandshakeCommand` | `RegistryPeerAdmissionQuery`, `RegistryDeviceEnabledQuery`, `VerifyRegistryHandshakeQuery`, `LoadDeviceRegistryQuery`, `LoadRegistryEventsQuery` |
| External references | `UpsertExternalRefCommand` | — |
| Integration | `ReceiveInboxReceiptCommand`, `PersistNodeAuthorizationCommand`, `PersistIntegrationGrantCommand`, `AcquireIntegrationRefreshLeaseCommand`, `PublishIntegrationCredentialEnvelopeCommand`, `ApplySignedIntegrationFrameCommand` | `IntegrationReadQuery`, `ValidateOutboundSignedFrameQuery` |
| Legacy mutation | `LegacyDeleteCommand`, `LegacyGraphWriteCommand`, `TrackedAppUpsertCommand`, `TrackedAppDeleteCommand` | — |
| Object mutation | `LocalBookCoverCommand`, `CanonicalGameUpsertCommand`, `LocalObjectDeleteCommand`, `LocalObjectUpsertTask` | — |
| Sync and policy | `CommittedFeedProgressCommand`, `EnsureUsageSequenceMigratedCommand`, `ApplySyncEntityCommand`, `ApplyAuthenticatedSyncEntityCommand`, `SetSyncKeyValueCommand`, `SetLocalSyncKeyValueCommand`, `SaveLocalSyncProfileCommand`, `AcceptPeerSyncPolicyCommand` | `LoadActiveLocalSyncProfileQuery`, `LoadAcceptedPeerSyncPolicyQuery`, `LoadSyncProfileForSyncQuery`, `LoadPolicyBoundEntitiesPageQuery`, `PolicyBoundFeedDeliveryPageQuery`, `LoadEntitiesQuery`, `UsageCompleteThroughQuery`, `CommittedFeedSnapshotQuery`, `CommittedFeedPageQuery`, `CommittedFeedDeliveryPageQuery`, `GetSyncKeyValueQuery`, `AuthorizedTransportKeyQuery` |
| System | `ClearAllCommand`, `DeleteTrashedCommand`, `LoadOrGenerateIrohSecretKeyCommand`, `CompactUsageSyncLogCommand` | `SystemReadQuery` |
| Task merge | `AppendTaskMergeOperationCommand`, `PrepareTaskMergeBatchCommand`, `AcknowledgeTaskMergeBatchCommand`, `ApplyTaskMergeBatchCommand` | `TaskConflictSetQuery` |
| Type registry and links | `RegisterPackageTypesCommand`, `UpsertObjectTypeCommand`, `DeleteObjectTypeCommand`, `UpsertObjectLinkCommand`, `DeleteObjectLinkCommand`, `ReplaceObjectLinkCommand` | `TypeReadQuery` |
| Usage | `LocalUsageWriteCommand` | `UsageReadQuery` |
| Object reads | — | `ObjectReadQuery` |
| Maintenance | `RestoreSnapshotCommand` | — |

`LocalObjectUpsertTask` is an actor task rather than a `DbCommand` after commit `219f7624`; the census therefore reports 44 `DbCommand`s, 24 `DbQuery`s, and one maintenance command. It remains in the operation-family map because it still runs through the same Store writer contract. The maintenance operation is counted separately from typed read/write commands, for 69 total typed operations.

## Cutover constraints

- Core keeps admission, fairness, cancellation, reader barriers, maintenance scheduling, and Engine error mapping.
- Store owns the sole production SQLite connection, SQL, schema, read connections, transaction/savepoint execution, bootstrap persistence, snapshots, and recovery.
- A Store callback may use only a named capability. It cannot receive a `Connection`, `Transaction`, statement, SQL string, raw row, or generic query function, and it cannot await.
- A single SQLite database remains in use. Each existing command keeps its current transaction scope, including coupled object/task rows, revisions, receipts, journals, and sync-feed state.
- Sync and network work happens after the Store transaction ends. Core composes Store, Application, Sync, and Net and preserves the Engine JSON and wire contracts.
- Remove each Core SQL facade with its last caller. Do not add a Core compatibility facade around Store.

## Remaining evidence gaps

- The current source scanner counts 69 typed operations, but ownership mapping groups enum variants under their containing operation family. The map covers the variants found in the command definitions; regenerate the census after every command-tree change.
- Canonical migration retirement is gated by postconditions and runs inside the `ark_canonical_contracts` savepoint in `db/open.rs`; preserve this caller-owned boundary when moving it.
- Confirm restore and startup actor error mapping against the current platform tests during the final migration unit.
- The SQL schema-level behavior of generic object/type read helpers still needs a before/after fixture inventory when those families move. The current query and response compatibility tests remain the behavioral oracle.
- Current Core still owns raw SQLite access. The source gate and Cargo dependency check must remain red until the full cutover is complete.
