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
- [x] Finish registry admission on Iroh and direct WebSocket client/server paths. The room relay does not support authenticated peer-addressed registry control and remains rejected before user data, as allowed by the current Linear issue scope.
- [x] Add Engine operations for synchronized rename and global disable; enforce disable at signed command and SQLite apply boundaries
- [x] Revoke ready sessions and queued data on Iroh and direct WebSocket paths; propagate verified registry events and deliver control history to offline peers before data readiness.
- [ ] Independently re-review the complete KOS-429 diff and pass the full repository gates. Linear marks KOS-429 Done.

Invariant: before a peer completes its durable enrollment, no transport frame may carry user data and no inbound user-data event may mutate storage. Pairing control and the Hello required to establish identity remain available. A future session remains closed to data until signed registry exchange produces `ReadyPeer`.

Candidate A (Luna): transport-owned per-connection phases with opaque pending, enrolled, and ready handles. The transport writer and reader classify every message at the connection boundary. Registry exchange returns `ReadyPeer`, and sync services require it. Direct SyncServer uses `ReadyPeer` bound to its connection ID, device ID, and signed registry session ID. Iroh uses the authenticated endpoint and registry session held by `RelayRegistryAdmission`. The direct SyncClient path applies live events through its registry admission state.

Candidate B (small cutover): keep existing RelaySync APIs, handle Hello with initial sync deferred, persist enrollment, then bind the Iroh endpoint and send consent/VersionVector. The transport writer already blocks ordinary frames until bound; the reader's single event loop serializes inbound delivery while enrollment awaits. Binding after the transaction avoids exposing a half-enrolled endpoint, at the cost of persisting a valid consent fact even if the in-memory endpoint remaps before binding.

Rubric: (1) outbound and inbound data remain closed during enrollment failure; (2) the design extends to signed registry exchange and shared `ReadyPeer` without changing ownership again; (3) restart/retry behavior is idempotent and durable facts are not rolled back; (4) callers cannot bypass the gate with another send path; (5) API and code changes stay proportional to the invariant.

Cross-judge selected A, 21/25, over B, 16/25. The accepted graft is B's durable order: Hello and consent, SQLite enrollment commit, PairingAccepted, then data admission and VersionVector. Direct SyncServer gates reader dispatch and socket writes with `ReadyPeer`, including a durable admission recheck. Iroh gates queued reads and writes on registry admission. The custom room relay does not support registry admission and is rejected before user data.

Historical validation above predates the final transport work. Current focused checks pass for live three-device Iroh rename/disable, third-peer revocation, direct client/server registry admission, and offline Iroh catch-up. The complete `ark-core --lib` suite passes 540 tests; strict Clippy, formatting, diff, and source-size checks pass. The room relay stays fail-closed because it cannot authenticate peer-addressed control messages. Linear marks KOS-429 Done.
