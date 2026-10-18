# ARK Stage 2 boundary decisions

## Target ownership

`ark-domain` owns pure values and deterministic rules. `ark-application` owns use cases and persistence capabilities. `ark-store` owns SQLite schema, connections, the single writer, SQL adapters, queries, and snapshot lifecycle. `ark-sync` owns transport-neutral session state, exchange rules, and authorization decisions. `ark-net` owns byte transport, endpoint identity, and live connection checks. `ark-core` composes those components and preserves the Engine JSON API.

The database remains one logical store and one production writer. Each cross-row mutation keeps its existing single SQLite transaction. Network awaits never run inside that transaction. Caller migration and old-path deletion happen together.

## First extraction seam: outbound integration authorization

Today `ark-net` receives `Arc<dyn SyncRuntimeStorage>` and queries it from Iroh writer tasks before sending a signed integration frame. That gives the transport a broad persistence interface and makes the dependency graph point through application data access.

The transport only needs a decision for one recipient-bound frame at the last responsible moment. `ark-sync` therefore owns `OutboundIntegrationFrameAuthorizer`, whose implementation depends only on the existing `IntegrationReplicationBackend` capability. The composition root narrows the aggregate storage object to that one port. Iroh receives only the authorizer, validates the frame immediately before writing, and separately checks that the recipient still maps to the live Iroh endpoint. It no longer receives the storage object or storage context.

```rust
#[async_trait::async_trait]
pub trait OutboundIntegrationFrameAuthorizer: Send + Sync {
    async fn authorize(
        &self,
        frame: &SignedSyncEnvelope,
        transport_public_key: &str,
    ) -> Result<(), String>;
}

pub trait SyncTransport: Send + Sync {
    fn set_outbound_authorizer(
        &self,
        authorizer: Arc<dyn OutboundIntegrationFrameAuthorizer>,
    ) -> Result<(), String>;
}
```

This is a capability seam, not a second authorization policy. The existing SQLite-backed validator remains authoritative. A missing authorizer fails closed. Authorization remains immediately before serialization/write, preserving revocation timing; the current endpoint-generation checks remain in place after the async authorization call.

## Usage journal SQL ownership move

The Engine's maintenance request remains unchanged. Its typed database command still enters the existing single writer and passes that command's transaction to the usage journal implementation. The SQL for calculating provable per-device cursors and deleting superseded rows now lives in `ark-store::usage_sync_log`; the persisted version-vector key is defined by `ark-store`. The old `ark-core::db` compaction exports are removed, and the sync query, maintenance command, and characterization tests call the store module directly.

The call graph is `ArkService -> DbHandle command -> ark-store usage journal SQL`. It preserves the transaction boundary and bounded batch behavior. This vertical SQL move does not complete the writer-executor extraction: `DbHandle`, admission, scheduling, and snapshot/restore lifecycle remain in `ark-core` and are still owned by the next storage-boundary unit. Store-local tests prove its cursor logic can run without `ark-core`; core tests keep the end-to-end compaction and sync-cursor behavior pinned.

## `sync_kv` SQL ownership move

The generic key-value SQL helpers now live in `ark-store::sync_kv`, and every production and test caller uses that module. `ark-core::db` no longer exports the SQL functions. Reads still return an absent value for a missing key, and writes retain the existing SQLite upsert semantics. Existing caller transactions and command authorization remain in `ark-core`; the store only owns the SQL operation. Store-local tests cover missing reads and replacement of a persisted key.

## Committed-feed watermark ownership move

The SQL that computes the durable committed-feed watermark now lives in `ark-store::committed_feed`. Feed snapshots, bootstrap progress, and snapshot restore call the store-owned query directly. The value still takes the maximum of the persisted watermark, retained feed rows, and `sqlite_sequence`, so pruning and older databases preserve the existing lower bound. Snapshot restore remains an `ark-core` use case until the complete restore lifecycle moves as one boundary.

## SQLite open and backup primitives

`ark-store::open` owns the integrity check and unpublished chunked SQLite backup. Snapshot manifest publication was initially above the low-level backup primitive; the complete manifest, validation, restore, journal, and recovery lifecycle now lives in `ark-store::snapshot`. Core coordinates canonical migration phases and adapts the user-facing backup request. SQLite compatibility migrations live in `ark-store::schema::migrations`.

## SQLite compatibility migrations

`ark-store::schema::migrations` owns compatibility DDL and data backfills for existing SQLite databases, including the legacy task-merge journal, device-registry columns, integration credential fencing, and usage runtime totals. It also bootstraps committed-feed heads through the store-owned feed module. Core still creates the baseline tables and coordinates phase 2 type-registry work and phase 3 canonical migrations. This keeps SQLite compatibility SQL with the database owner without moving application-level canonical migration policy into storage. The existing order remains create tables, apply compatibility backfills, then run phase 2 and phase 3 work.

## Direct WebSocket dialing

## Authorized-node SQLite lookup

`ark-store::integration_replication::load_authorized_node` owns the row query, SQLite integer decoding, persisted-status decoding, and validation of one authorized-node record. Core replication policy continues to decide how that record affects grants, leases, credential publication, and signed-frame admission. All callers use the store query directly; the old `ark-core::db::load_authorized_node` facade is removed. The query remains inside the caller's existing SQLite connection and transaction.

`ark-store::integration_replication::load_integration_refresh_lease` follows the same boundary for persisted refresh lease rows: SQLite decoding, non-negative integer conversion, and record validation live with storage. Core still enforces current holder, grant, generation, HLC, and fencing policy using this record. Its `ark-core::db` facade is removed and callers keep their existing connection and transaction.

`ark-store::integration_replication::load_integration_configuration` owns configuration row decoding and validation. It also retains the compatibility behavior that scrubs legacy opaque values from the public numeric `sync_cursor` column before returning a configuration; opaque provider cursors remain in the encrypted credential envelope. Core keeps integration authorization policy. The writer census now classifies that compatibility update in Store.

`ark-store::integration_replication::load_integration_credential_envelope_record` owns SQLite row decoding, integer validation, metadata JSON decoding, and envelope validation for a specific generation; its `load_latest_integration_credential_envelope_record` query selects the latest recipient-bound generation. Core keeps the recipient node and grant authorization check after loading, so moving persistence decoding does not weaken revocation behavior. Existing Core loader functions remain policy adapters and preserve caller signatures.

`ark-store::integration_replication` also owns integration-grant row decoding and the insert/upsert statement. Core still checks that active grants match an active authorized node and the node's current key epoch; it also enforces monotonic grant epochs, no same-epoch revival after revocation, key binding, HLC ordering, and version-vector updates in the existing transaction.

Store owns the integration-configuration insert/upsert and encoding of its JSON and SQLite integer fields. Core retains provider/account identity immutability and HLC ordering checks, and continues to include the upsert plus version-vector bump in the existing transaction.
Core performs this identity check through the Store configuration loader inside its current savepoint. The loader's legacy opaque-cursor cleanup remains part of that transaction, so a rejected identity/HLC update rolls the cleanup back with the savepoint.

Store owns authorized-node row encoding and its insert/upsert alongside the read adapter. Core uses the validated Store record to enforce grant-epoch monotonicity, prevent same-epoch revival and key changes, and check HLC order before persisting and bumping the version vector in the existing transaction.

Store owns refresh-lease row encoding and upsert. Core validates the holder node and active grant, then applies generation and fencing-token monotonicity; the caller's version-vector bump and row write remain in the same transaction. Equal-fence replacement stays an explicit Core policy input for the existing acquisition flow.

Store owns credential-envelope decoding, maximum-generation lookup, and upsert SQL. Core validates recipient and issuer authorization, lease holder/generation/fence, integration-wide generation monotonicity, and per-envelope HLC ordering before it writes the row and bumps the version vector in one transaction.

Peer export keeps recipient authorization in Core. After Core verifies the live node and grant, it supplies that node's grant epoch to a Store query that returns only matching envelope generations in ascending order; Core then validates each loaded envelope before including it.

`ark-net::websocket::race_connect` owns racing direct WebSocket addresses, the five-second per-address timeout, and cancellation of losing attempts. `ark-core::SyncClient` keeps peer selection, reconnect delay, authentication, and protocol exchange. Keeping the returned socket type in `ark-net` makes this a concrete transport API rather than a sync policy interface. Moving the whole client into `ark-sync` would also move listener and socket details into the policy layer, so that larger cut remains separate.

`ark-net::websocket::Listener` now owns the TCP listener and WebSocket server handshake as one transport operation. Core still selects between accepting a peer and its ping timer, then owns peer identity, admission, connection tasks, and shutdown. This moves concrete socket setup to `ark-net` without moving session lifecycle into the network package.

## Client Hello construction

`ark-sync::client_session::build_client_hello` owns the transport-neutral handshake offer, nonce generation, and authentication HMACs. Core supplies a named `ClientHelloRequest` with the local identity and addresses plus platform/version metadata from `ark-net`; it still owns socket I/O and the subsequent reconnect loop. The request struct keeps this cross-crate boundary explicit and prevents a growing list of positional handshake arguments.

## Snapshot and restore ownership decision

The snapshot unit is one lifecycle, not a bag of filesystem helpers: it validates a caller-supplied backup identifier, stages an immutable source, verifies schema and integrity, protects device identity and committed-feed monotonicity, writes a durable recovery capsule and intent, applies through SQLite Online Backup, verifies the result, rolls back on failure, and reports entity counts. Keeping only file helpers in `ark-store` would leave failure ordering and recovery split across crates, so the full lifecycle belongs in `ark-store::snapshot`.

Candidate A moves the complete lifecycle and private helpers into `ark-store`, while `ark-core::db` re-exports the existing request-facing types/functions. This preserves Engine signatures and keeps staging, manifest, restore journal, rollback, and recovery ordering under one owner. Candidate B moves file access and the journal but leaves restore orchestration and identity policy in `ark-core`; this keeps a storage concept dependent on the application and creates a cross-crate recovery protocol, so it is rejected. Device identity reapplication stays as an explicit storage invariant because its keys and registry rows are persisted SQLite state; no transport or sync policy enters the store.

Implementation constraints: keep the same backup directory and manifest format; retain no-follow file opening and directory sync behavior; preserve stale committed-feed refusal and registry event history; keep every restore operation under existing single-writer admission/barrier; keep the public Engine JSON shape unchanged. Store tests must use store-owned schemas/fixtures so `ark-store` remains independently buildable. Core tests should exercise the public lifecycle through `ark_store::snapshot` and keep actor-level ordering tests at the command boundary.

The implementation moved the snapshot files into `ark-store::snapshot`, added only the filesystem/hash/random dependencies needed by that lifecycle, and moved the persisted local-device-ID key beside `sync_kv`. `ark-core::db` re-exports the existing public request types and operations. Failure-injection helpers are available only through the `ark-store/test-support` feature enabled by `ark-core` dev-dependencies; they do not enter the production API. The store's journal tests run against store-owned SQLite fixtures, and actor recovery remains tested through the production `DbHandle` startup path.

## Single-writer ownership decision

The existing actor combines two responsibilities. Its ledger enforces trusted-source quotas, command-class fairness, admission reservations, cancellation, and the maintenance barrier. Those rules decide which Engine/sync request runs and when readers must pause. Its task layer owns the SQLite connection and transaction begin/commit behavior. The first group is request scheduling; the second is persistence execution.

Candidate A moves the actor, typed command context, source identities, and scheduler wholesale into `ark-store`. Reject it: this would make the storage package own Engine and sync admission policy and error types. Candidate B gives `ark-store` the connection-owning `SqliteWriter` and its transaction/maintenance execution methods while the existing Core actor keeps scheduling, read admission, barriers, and typed command dispatch. Choose B: it keeps the storage API small, enforces transaction boundaries at the storage owner, and avoids an inverted dependency. A later move of source quotas belongs in `ark-application` only if it can be expressed independently of SQLite and without changing current fairness or backpressure behavior.

The writer contract is synchronous and single-threaded by construction: one `SqliteWriter` owns one `rusqlite::Connection`; immediate operations begin one SQLite transaction, commit only on success, and report begin/operation/commit failures distinctly; maintenance receives exclusive mutable access only after the existing barrier is held. There is no await-capable callback, so network work cannot occur inside a transaction. `ark-core` retains the same public request and error mapping.

## Stage 2 sequence

1. Remove broad storage access from transports through the narrow sync-owned authorization capability; test allow, deny, missing authorizer, endpoint replacement, and feature builds.
2. Move SQLite adapters, schema initialization/migrations, writer executor, and snapshot lifecycle into `ark-store` as a single ownership boundary. Preserve one writer and transaction scopes; migrate callers and delete old internal paths in the same unit.
3. Move remaining sync session orchestration into `ark-sync`; keep concrete WebSocket/Iroh handling in `ark-net`, and composition plus the JSON API in `ark-core`.
4. Remove obsolete facades and dependencies; run the graph and API compatibility gates, then full CI on the stage commit.

Every unit ends in a focused production-path test, diff inspection, and commit. Cross-platform claims come from CI on the exact published SHA.
