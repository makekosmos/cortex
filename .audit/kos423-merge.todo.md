# KOS-423 merge contract

## Architect phases

- [x] Ground the current task-object write, revision, sync, and SQLite paths.
- [x] Sketch two structurally distinct merge designs with caller examples, types, signatures, and module ownership.
- [x] Cross-judge the designs and screen them for stale-write loss, unsafe deletion, unbounded history, and wire incompatibility.
- [ ] Implement the smallest accepted conflict-preserving contract and record any deviation.
- [ ] Verify concurrent edits, deletion races, duplicate/reorder/restart, and stale resolution through public APIs and SQLite.
- [ ] Add an operation-specific committed-feed representation and negotiated capability before enabling peer delivery.
- [ ] Reject legacy full-object writes for operation-enabled objects so LWW cannot erase stored variants.
- [ ] Scrap and redesign if implementation needs repeated exceptions outside the accepted shape.

## Arena phases

- [x] Frame a 3-6 point rubric and provide the same source-grounded prompt to each candidate.
- [x] Fan out independent candidates into separate output directories.
- [x] Cross-judge candidates after both finish.
- [x] Pick a base against each rubric criterion.
- [x] Graft only useful ideas from the losing design.
- [x] Verify the synthesized design against current callers and stored task fixtures.

## Throughput checkpoint

- [x] After each implementation commit, record the hypothesis, before-state, production test result, and next acceptance gap.
- Verified local persistence/API slice: a sealed SQLite actor stores task-field operations append-only, exposes common ancestors and concurrent variants, keeps unresolved parents pending, rejects malformed parent shapes before storage, replays operation IDs idempotently across reopen, and atomically requires the observed digest when an operation closes a conflict frontier. Before, `SyncEntity` carried only a last-writer object snapshot and no durable variants. The 17 matching domain, DB actor, and service tests pass; strict Clippy, formatting, diff, and source-size checks pass. An independent review replaced string-based error classification with typed storage errors. Remaining acceptance gap: normal task edits are not yet converted into these operations, and operations have no committed-feed/wire delivery or legacy-write fence, so this slice does not claim cross-device convergence.
- Full `ark-core --lib` validation first reported 539/540 because registry revocation treated an already-closed Iroh connection as an error. `disconnect_transport_peer` now treats that state as success and removes the stale endpoint. The full serialized suite passes 540/540 after the fix; strict Clippy, formatting, diff, and source-size checks pass.
- Architecture cross-judge chose a separate append-only task operation lane, not `ark_committed_feed`, because the latter coalesces entities and projects current state. `docs/ark-exocortex.md` records the one-way per-task fence, old-peer refusal, separate receive/apply progress, and policy-bound history backfill. Storage now assigns a local journal position to each accepted operation, backfills existing rows idempotently, and reads bounded pages at a pinned watermark after verifying stored payload digests. The 18 merge domain, DB actor, and service tests pass, including duplicate delivery, SQLite reopen, page identity, sequential positions, and rollback when journal persistence fails.
- The bounded-page storage port now reads through the authenticated sync-peer command path; its query checks the peer identity, page bounds, pinned watermark, canonical operation bytes, operation ID, and stored digest. `cargo nextest run -p ark-core --test-threads 1` passes 721/721 (one skipped), strict Clippy passes, and format, source-size, and diff checks pass. Parallel nextest runs exposed unrelated timing-sensitive failures in the policy Iroh test and the actor cancellation test; each failure passed in the complete sequential run, and the policy test also passed alone.
- Journal pages now carry the accepted receiver-policy identity. A stale identity is rejected, and active policies permit operation pages only when `core.task` is selected in `full` mode with no field filter. Metadata-only and filtered delivery are refused because operation history can expose values outside the current snapshot filter. The two SQLite actor page tests pass, strict Clippy passes, and formatting, source-size, and diff checks pass.
- Two independent production `ServiceState` instances now persist and read the same causal task operations in opposite arrival orders. The service API and SQLite adapter return identical title conflict sets with their common ancestor; a deadline edit remains a single variant on both replicas. The three service merge tests pass. This test delivers operation requests directly, so it does not prove the live sync transport, deletion/update races through SQLite, or the old-peer snapshot fence.

## Next implementation boundary

Source audit confirms the stored operation API is currently a sidecar: `AppendTaskMergeOperationCommand` appends/query conflict history but does not materialize the task, write an object receipt, or append the committed feed. Normal task edits therefore remain outside that history. The current feed stores revision locators and projects the latest object snapshot; it cannot preserve operation variants. Do not generate operations or enable their delivery as an isolated step.

- Local canonical task edits enter through `service/runtime/objects.rs::upsert_object` and `LocalCoreTaskWrite::execute`; the object projection, HLC, receipt, and snapshot committed-feed entry are committed in one SQLite actor transaction.
- The first mutation-seam phase is complete (commits `98641825` and `6f205e3c`): local canonical writes/deletes, legacy `todo` writes/deletes, and inbound task snapshots/deletes all pass through `TaskMutationSession` in `application/task_mutation.rs`. Compatibility commands do not use `LocalCoreTaskWrite`, but they already use the shared transactional seam. This preserves existing projection, receipt, link, and feed behavior while generation remains disabled. The seam is not yet operation-aware and does not itself produce merge operations.
- Inbound full snapshots still overwrite the projection through both `AuthenticatedCoreTaskWrite::execute` and `apply_canonical_object_entity`; include full `object`, compatibility `todo`, and tombstone/delete paths in the legacy-write fence.
- Safe sequence: (1) converge canonical, legacy, and delete task writes on one transactional mutation seam while preserving current projection, receipts, links, and feed behavior, with operation generation still disabled — complete; (2) add authenticated operation delivery and inbound apply with separate per-peer progress; (3) atomically fence legacy full-object writes and tombstones for operation-enabled tasks; (4) make the shared task mutation transaction emit and materialize operations; (5) enable generation and peer delivery only after the entire operation path is available. Do not create a mixed-mode window where the projection or peers can lose stored variants.
- Current accepted storage shape refines step (2): keep a dedicated operation journal and peer cursor rather than extending the coalescing committed feed. Operation positions are local delivery indexes, never causal order. Capability advertisement remains disabled until operation apply and delivery handlers exist.
- The owner clarified receiver policy: a device's type/content selection controls what it receives. Excluding a category stops transfers to that device and preserves local copies; other selected devices can still receive them. Adding a category back requires backfill. This policy is separate from merge history and deletion semantics.

## Candidate outputs

- `/tmp/kos423-candidate-a.md`: typed conflict set, common ancestor, explicit deletion variant, exact digest, and transactional resolution.
- `/tmp/kos423-candidate-b.md`: causal patch ledger with projection, replay, and checkpoint lifecycle.
- `/tmp/kos423-design-judge.md`: candidate scores and synthesis. Candidate A is the bounded contract; preserve B's causal patch idea as an implementation option, but do not commit to a ledger, backend, or GC policy before production evidence demands it.
- User product clarification: per-device type/content selection changes transfer only. It does not delete an existing local copy; enabling selection later requests backfill. This is independent of conflict resolution.
- Recovery dependency reconciled with KOS-428: operation IDs are collision-resistant random IDs persisted once; causal parent refs establish order. Stale restore stays refused by the committed-feed watermark. Add a recovery epoch only if a future design permits restoring behind that watermark.
- Production seam from read-only review: `SyncEntity`/committed feed publishes the materialized object, which erases causal variants. Keep the first operation store private to actor-backed typed API; do not feed-project it as an ordinary object. Cross-device delivery requires an operation payload/feed kind, capability negotiation, and a legacy-write fence in the same release sequence.
- Minimal product proof: two temporary SQLite databases with production actor commands; compatible title/deadline edits converge in either order; same-title edits expose both values; delete/edit remains explicit; duplicate/reordered/reopened replay is stable; a stale conflict-set hash rejects resolution with no changed rows or receipt; an operation-enabled object refuses legacy full-object sync.
