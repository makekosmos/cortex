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

KOS-429 remains the active implementation. Current code adds direct WebSocket challenge proof and registry paging; direct admission tests cover the pre-data barrier and reject out-of-range pages. A registry page that disables the local device now fails admission in direct WebSocket and RelaySync before data readiness. The Engine persists its configured local device ID and exposes a typed registry snapshot, rename, and disable API backed by signed SQLite commands. The current full `ark-core --lib` suite passes (474 tests), and `cargo check -p ark-core --all-targets --locked` passes. RelayTransport still has no registry capability or peer-addressed delivery, so legacy peers are rejected before data sync. Disables learned after a session is already ready still need to revoke live sessions and queues. Manager wiring, active registry update delivery, control-only offline catch-up, the relay disposition, independent final review, and full repository validation remain open. Finish and verify all KOS-429 paths before starting dependent issues.

Current execution units:
- [x] Revalidate current checkout and live KOS-429 acceptance criteria.
- [x] Run current full ark-core library suite and focused direct admission tests.
- [x] Reproduce and reject registry pages outside the declared page count before storage mutation.
- [x] Reject local-device disable discovered in a registry page before direct WS or RelaySync data readiness.
- [x] Expose Engine snapshot, rename, and disable operations backed by the local signed registry commands.
- [ ] Resolve remaining KOS-429 review findings, including transaction-level disable checks and session/queue revocation.
- [ ] Update Manager to use the Engine registry API and propagate committed events to connected peers.
- [ ] Establish the relay delivery contract; do not advertise registry admission while the relay broadcasts clear user data to unaddressed room members.
- [ ] Verify offline replay, restart, replay, multi-peer propagation, and all required repository gates; update Linear and commit the verified KOS-429 unit.
- [ ] Complete KOS-422 through KOS-428 in dependency order, updating this checklist and Linear after each verified unit.
