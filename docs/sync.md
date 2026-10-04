# Sync

ARK replicates user data peer-to-peer; the implementation lives in
`core/crates/ark-core/src/` (`sync_server*.rs`, `sync_client*.rs`, `db/sync/`,
`iroh_*`, `relay_*`, `beacon_*`).

## Transports

`transport_select::select_transport(use_iroh, relay_url)` picks one:

- **iroh** — p2p QUIC; wins when `use_iroh` is set even if a relay URL is
  also present.
- **relay** — WebSocket relay fallback (`relay_url`).
- **beacon** — LAN broadcast discovery for peer finding (`beacon_*` modules).

Wire messages are `snake_case` JSON. Do not weaken self-peer rejection or
routable-address filtering (`sync_server_peers.rs`, `net.rs`).

## Versioning

- **HLC** (`hlc.rs`): hybrid logical clocks stamp every mutation; conflict
  resolution compares `HLC` values.
- **Version vectors** (`db/sync/`): per-entity revisions. Every write to
  syncable data must call `ark_core::db::bump_sync_version_vector` — the
  write-boundary rule in `docs/write-boundary.md`.
- **Usage entities** (`tracked_apps`, `usage_sessions`, `usage_events`,
  `usage_days`) are *sequenced*: `bump_sync_version_vector` routes them to
  `ensure_usage_sequence_migrated` → `next_usage_sequence` →
  `record_usage_sequence`, storing per-device sequences in
  `usage_sync_versions` / `usage_sync_heads` / `usage_sync_log`.
- **Objects** keep revisions in `sync_kv` (`VERSION_VECTOR_KEY`) plus
  `sync_tombstones` for deletes; `get_object_revision` prefers the stored
  object revision when present.

## Rules for changes

- Additive only: new message fields must be optional; old peers must not
  break.
- Never bypass `record_local_*` / `bump_sync_version_vector` with a bare
  `conn.execute(INSERT/UPDATE/DELETE)` on syncable tables.
- Integration replication (`integration_replication`) uses signed envelopes
  (`SignedSyncEnvelope`) and explicit node grants — do not auto-trust peers.
