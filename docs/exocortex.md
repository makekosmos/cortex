# ARK: the data core behind the Atlas client

ARK is the user-data core that ships inside the Mundus Engine. One client talks
to it — Atlas — and every feature that touches user data goes through the
Engine's operation surface into `ArkService`. There is no package marketplace,
no `.kspkg` runtime, and no third-party workers: a capability that needs user
data is part of the client or the Engine, reviewed and released with them.

## Shape

```
Atlas (GPUI client)  ──HTTP/WS──▶  Engine (runtime/)
                                     │
                                     ├─ ArkHost  (runtime/src/ark_host.rs)
                                     │     └─ ArkService  (core/crates/kernel/src/service.rs)
                                     │
                                     ├─ kernel    request admission, actor, events
                                     ├─ store   SQLite: schema, writer, transactions
                                     ├─ sync    session policy, paging, registries
                                     ├─ net     iroh + relay + beacon transports
                                     └─ domain  pure rules and wire contracts
```

Dependencies point down only: `domain` has no internal dependencies,
`store` and `sync` sit on it, `net` is the transport layer, and
`kernel` composes all of them. `scripts/check-dependency-graph.mjs`
enforces the whitelist, so a new edge has to be argued for, not discovered.

## What each crate owns

- **domain** — types, canonical definitions, HLC stamps, task-merge and
  selective-sync rules, book-cover and canonical-game plans, and the storage
  ports (`sync_storage`, `replication_storage`, `registry_admission`) that store
  and sync both speak. Nothing here does I/O.
- **store** — everything that touches SQLite: the single writer, migration
  and schema, read capabilities, snapshot/restore, and the write plans that turn
  a request into one transaction.
- **sync** — policy for a sync session: hello/negotiation, page selection,
  inbound application, committed-feed delivery, registry exchange.
  `docs/sync.md` covers transports and versioning.
- **net** — iroh and WebSocket relay transports, beacons, address filtering.
  It may not reach storage: `check-dependency-graph.mjs` greps its sources
  for storage symbols.
- **kernel** — the only crate the Engine links against directly: request
  admission over a bounded command queue, the database actor, event fan-out,
  pairing, and the sync session wiring.

## Invariants that stay expensive to change

- **One database per device, owned by one process.** Only the Engine opens it;
  see `docs/write-boundary.md`.
- **One transaction per mutation.** Admission, validation and the SQL work for a
  write happen in one transaction on the writer connection, and no network await
  runs inside it.
- **Every syncable write bumps a version.** Writes go through
  `store::sync_versions`, which is what makes replication and tombstones
  correct after a crash.
- **Wire messages are frozen at `snake_case` JSON** with the protocol version
  carried in the offer; compatibility is negotiated, never assumed.
- **Inbound data is re-authorized.** A peer's frame passes the same validation
  as a local one before it reaches the writer.

## Client boundary

`ArkService::call` is the operation surface: `{"op","params"}` in,
`{"ok","data","error"}` out. The Engine exposes it to Atlas over HTTP and
WebSocket; in-process callers (agents, dictation) use `ArkHost`.
Adding an operation means adding it there, not opening a second path to the
database.

Releases are one Engine build plus the client build from the same commit; the
nightly pipeline stages both, and the installer upgrades them together.
