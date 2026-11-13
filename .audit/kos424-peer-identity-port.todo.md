# KOS-424 peer identity and sync policy application ports

- [x] Move local/accepted sync profile values and policy digest into
  `ark-application`; keep SQLite persistence in core.
- [x] Move `PeerIdentityBackend` and its registry page result into
  `ark-application` using only application/domain types.
- [x] Preserve `ark_core::data_platform::*`, root `PeerIdentityBackend`, and
  `ark_core::replication_storage::PeerIdentityBackend` paths as facades.
- [x] Run full serial ark-domain/ark-application/ark-core tests, strict Clippy,
  all-target check, dependency tree, formatting, source-size, and diff checks.
- [x] Commit this boundary independently; keep KOS-424 open.

The policy values and peer identity port are now owned above `ark-domain` and
below the SQLite adapter. Transport, admission orchestration, signed envelopes,
and SQL reads/writes remain in core for later cohesive moves.
