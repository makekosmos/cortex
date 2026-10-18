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

## Stage 2 sequence

1. Remove broad storage access from transports through the narrow sync-owned authorization capability; test allow, deny, missing authorizer, endpoint replacement, and feature builds.
2. Move SQLite adapters, schema initialization/migrations, writer executor, and snapshot lifecycle into `ark-store` as a single ownership boundary. Preserve one writer and transaction scopes; migrate callers and delete old internal paths in the same unit.
3. Move remaining sync session orchestration into `ark-sync`; keep concrete WebSocket/Iroh handling in `ark-net`, and composition plus the JSON API in `ark-core`.
4. Remove obsolete facades and dependencies; run the graph and API compatibility gates, then full CI on the stage commit.

Every unit ends in a focused production-path test, diff inspection, and commit. Cross-platform claims come from CI on the exact published SHA.
