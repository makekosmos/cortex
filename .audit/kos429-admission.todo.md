# Pairing data-admission gate

- [x] Ground current Iroh binding, writer authorization, and RelaySync event ordering
- [x] Frame the race and failure invariant
- [x] Compare two whole-shape admission designs
- [x] Select the transport-owned phase model (cross-judge: 21/25 vs 16/25); graft commit-before-activation ordering
- [x] Implement outbound and inbound pre-admission gates
- [x] Add WebSocket challenge proof and signed registry paging before data readiness
- [x] Replace direct SyncServer's mutable authentication boolean with a connection/device/registry-session-bound `ReadyPeer`; recheck admission before inbound dispatch and outbound user-data writes
- [x] Keep disabled-device rejection and transaction-level inbound authorization checks
- [x] Prove pairing enrollment failure leaves no admitted peer or partial event
- [x] Retry PairingAccepted after send failure without duplicating the committed enrollment
- [x] Run focused and full ark-core validation, lint, formatting, and size gate
- [ ] Finish equivalent registry admission for remaining Iroh, relay WebSocket, and direct SyncClient paths; relay broadcast cannot safely deliver registry control yet
- [x] Add Engine operations for synchronized rename and global disable; enforce disable at signed command and SQLite apply boundaries
- [ ] Revoke every ready session, feed, and queued frame on all transports; propagate committed registry events and deliver control history while data sync is filtered/offline
- [ ] Independently re-review the complete KOS-429 diff, pass the full repository gates, update Linear, and commit the verified step

Invariant: before a peer completes its durable enrollment, no transport frame may carry user data and no inbound user-data event may mutate storage. Pairing control and the Hello required to establish identity remain available. A future session remains closed to data until signed registry exchange produces `ReadyPeer`.

Candidate A (Luna): transport-owned per-connection phases with opaque pending, enrolled, and ready handles. The transport writer and reader classify every message at the connection boundary. Registry exchange returns `ReadyPeer`, and sync services require it. Direct SyncServer now uses a local `ReadyPeer` bound to its connection ID, device ID, and signed registry session ID; this does not yet prove equivalent gating across Iroh, relay WebSocket, and SyncClient paths.

Candidate B (small cutover): keep existing RelaySync APIs, handle Hello with initial sync deferred, persist enrollment, then bind the Iroh endpoint and send consent/VersionVector. The transport writer already blocks ordinary frames until bound; the reader's single event loop serializes inbound delivery while enrollment awaits. Binding after the transaction avoids exposing a half-enrolled endpoint, at the cost of persisting a valid consent fact even if the in-memory endpoint remaps before binding.

Rubric: (1) outbound and inbound data remain closed during enrollment failure; (2) the design extends to signed registry exchange and shared `ReadyPeer` without changing ownership again; (3) restart/retry behavior is idempotent and durable facts are not rolled back; (4) callers cannot bypass the gate with another send path; (5) API and code changes stay proportional to the invariant.

Cross-judge selected A, 21/25, over B, 16/25. The accepted graft is B's durable order: Hello and consent, SQLite enrollment commit, PairingAccepted, then data admission and VersionVector. Direct SyncServer now gates both reader dispatch and socket writes with `ReadyPeer`, including a durable admission recheck. Other transports still need the same boundary before this design is complete.

Historical validation above predates registry exchange and is not current evidence. On 2026-10-16, current `cargo nextest run -p ark-core --no-fail-fast` passed 651 tests with 1 skipped; Clippy, format, source-size, and diff checks also passed after the direct `ReadyPeer` change. These gates do not prove KOS-429 complete: relay admission, remaining Iroh/SyncClient gates, live registry fanout, and control-only offline catch-up are still open.
