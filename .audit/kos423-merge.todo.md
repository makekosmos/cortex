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

- [ ] After each implementation commit, record the hypothesis, before-state, production test result, and next acceptance gap.
- Verified local persistence/API slice: a sealed SQLite actor stores task-field operations append-only, exposes common ancestors and concurrent variants, keeps unresolved parents pending, replays operation IDs idempotently across reopen, and atomically requires the observed digest when an operation closes a conflict frontier. Before, `SyncEntity` carried only a last-writer object snapshot and no durable variants. The 16 matching domain, DB actor, and service tests pass; strict Clippy, formatting, diff, and source-size checks pass. An independent review replaced string-based error classification with typed storage errors. Remaining acceptance gap: normal task edits are not yet converted into these operations, and operations have no committed-feed/wire delivery or legacy-write fence, so this slice does not claim cross-device convergence.

## Candidate outputs

- `/tmp/kos423-candidate-a.md`: typed conflict set, common ancestor, explicit deletion variant, exact digest, and transactional resolution.
- `/tmp/kos423-candidate-b.md`: causal patch ledger with projection, replay, and checkpoint lifecycle.
- `/tmp/kos423-design-judge.md`: candidate scores and synthesis. Candidate A is the bounded contract; preserve B's causal patch idea as an implementation option, but do not commit to a ledger, backend, or GC policy before production evidence demands it.
- User product clarification: per-device type/content selection changes transfer only. It does not delete an existing local copy; enabling selection later requests backfill. This is independent of conflict resolution.
- Recovery dependency reconciled with KOS-428: operation IDs are collision-resistant random IDs persisted once; causal parent refs establish order. Stale restore stays refused by the committed-feed watermark. Add a recovery epoch only if a future design permits restoring behind that watermark.
- Production seam from read-only review: `SyncEntity`/committed feed publishes the materialized object, which erases causal variants. Keep the first operation store private to actor-backed typed API; do not feed-project it as an ordinary object. Cross-device delivery requires an operation payload/feed kind, capability negotiation, and a legacy-write fence in the same release sequence.
- Minimal product proof: two temporary SQLite databases with production actor commands; compatible title/deadline edits converge in either order; same-title edits expose both values; delete/edit remains explicit; duplicate/reordered/reopened replay is stable; a stale conflict-set hash rejects resolution with no changed rows or receipt; an operation-enabled object refuses legacy full-object sync.
