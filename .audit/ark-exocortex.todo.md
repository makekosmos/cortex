# ARK exocortex foundation

- [x] Read poteto-mode Principles
- [x] Phase A: Frame
- [x] Phase B: Design the workflow
- [x] Phase C: Run the loop
  - [x] Create Linear epic and dependency-ordered child issues; preserve complete local draft if connection pending
  - [x] Ground StorageBackend consumers and compare extraction contracts
  - [x] Review first extraction sketch; implement behavior-preserving seam
  - [x] Add and verify source-name gate with shrinking legacy baseline
  - [x] Integrate independent units; run relevant Rust and script checks
  - [x] Obtain independent review and resolve actionable findings
- [x] Phase D: Keep the audit trail
- [x] Phase E: Verify and hand back

Definition of done for this initial delivery: reviewed architecture and epic with testable children; a verified, behavior-preserving first structural change and source-name guard. This does not claim completion of later schema, merge, blob or protocol work.

Throughput checkpoint: 3 gpt-6-luna workers, source ownership isolated by worktree/output directory. Reuse existing Cargo target configuration; do not parallelize heavyweight Cargo builds. Linear connection does not block local implementation.

## Full KOS-412 execution

Live Linear status checked issue-by-issue on 2026-10-16 for `KOS-413` through `KOS-429`: 9 Done, 1 In Progress, 7 Backlog (17 total). `list_issues(parentId=KOS-412)` omitted KOS-413..416 and KOS-418..419 despite their parent field; direct `get_issue` confirmed all six are children and Done. The foundation checklist above records the initial delivery only. The full goal remains open until every issue and the epic acceptance criteria pass.

- [x] KOS-413 through KOS-421: contract census, source structure, storage port, application transaction, DB executor, committed feed, and protocol negotiation (9/17 epic issues).
- [ ] KOS-422: per-device sync policy, local eviction, and backfill.
- [ ] KOS-423: merge behavior and retained conflict variants.
- [ ] KOS-424: extract proven domain, application, store, sync, and net crates.
- [ ] KOS-425: dependency whitelist and desktop/mobile build matrix in CI.
- [ ] KOS-426: explicit application scopes and approximate volume preview.
- [ ] KOS-427: separate blob metadata and content contract.
- [ ] KOS-428: backup, restore, and sync-identity recovery tests.
- [ ] KOS-429: signed device registry exchange, rename, disable, and the `ReadyPeer` data gate.

KOS-429 remains the active implementation. Direct WebSocket challenge proof, signed registry paging, and Engine/Manager registry operations exist. This step replaces the direct server's mutable `authenticated` flag with a connection/device/registry-session-bound `ReadyPeer`; inbound dispatch and outbound writes recheck durable admission. Revocation closes data readiness and lets the reader lifecycle deliver disconnect cleanup. Full `ark-core` nextest passes (651 passed, 1 skipped), Clippy, format, source-name, and diff checks pass. RelayTransport still lacks registry capability and peer-addressed delivery, so it cannot safely provide registry admission. Post-ready registry fanout, control-only offline catch-up, relay behavior, independent review, and full repository validation remain open. Finish and verify every KOS-429 path before starting dependent issues.

KOS-422 partial progress: device-owned profiles now have an Engine get/set API and are read/written through DB actor commands. Revisions are monotonic per device, active replacement is atomic, and SQLite reopen plus six-device isolation are tested. This does not yet exchange the receiver's policy with peers, bind network progress/ACKs to the policy revision and digest, or implement safe eviction and receipt-aware backfill.

Current execution units:
- [x] Revalidate current checkout and live KOS-429 acceptance criteria.
- [x] Run current full ark-core library suite and focused direct admission tests.
- [x] Reproduce and reject registry pages outside the declared page count before storage mutation.
- [x] Reject local-device disable discovered in a registry page before direct WS or RelaySync data readiness.
- [x] Expose Engine snapshot, rename, and disable operations backed by the local signed registry commands.
- [x] Replace direct SyncServer's mutable admission flag with a connection/device/registry-session-bound `ReadyPeer`; recheck inbound and outbound data at the socket boundary.
- [x] Keep write authorization and existing Engine/Manager registry operations backed by durable signed registry commands.
- [ ] Verify revocation cleanup and queued-frame behavior across remaining transports.
- [ ] Propagate committed registry events to connected peers and implement control-only offline catch-up.
- [ ] Establish the relay delivery contract; do not advertise registry admission while the relay broadcasts clear user data to unaddressed room members.
- [ ] Verify offline replay, restart, replay, multi-peer propagation, and all required repository gates; update Linear and commit the verified KOS-429 unit.
- [ ] Complete KOS-422 through KOS-428 in dependency order, updating this checklist and Linear after each verified unit.
