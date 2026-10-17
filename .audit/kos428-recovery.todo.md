# KOS-428 recovery design and delivery

- [x] Ground the current SQLite, actor, snapshot, identity, and shutdown paths.
- [x] Frame two structurally distinct recovery-epoch designs and grade them against explicit safety criteria.
- [x] Cross-judge both candidates and select one design.
- [ ] Implement recovery identity/resnapshot behavior against the selected shape.
- [x] Preserve unsent local changes or fail restoration before replacing the live DB.
- [x] Persist a monotonic committed-feed watermark and refuse stale restore before touching live data.
- [ ] Verify recovery with two persistent SQLite peers, failure injection, and supported-platform durability evidence.
- [ ] Reconcile the final contract, Linear acceptance, tests, and commits.

## Grounding

Current SQLite is opened with WAL and `synchronous=FULL`; ADR-001 documents that successful commits still depend on OS/filesystem/hardware flush behavior. The backup API creates a consistent snapshot and a manifest validates size, database hash, schema identity, and SQLite integrity. Restore is an actor maintenance operation with a rollback capsule and restart of sync after success. The capsule repairs an interrupted restore; it does not protect sync identity from restoring an older valid snapshot.

The identity values that can roll back with a DB snapshot include HLC high-watermark, origin/change sequence, version vectors, usage sequence, committed-feed positions, and operation receipts. The stable iroh secret/device identity is preserved in normal snapshot restore. A peer can therefore already know a change identity which the restored DB may generate again. No recovery epoch is exchanged by peers today.

The actor supports explicit Ready/Recovering/Failed lifecycle. SQLite busy remains retryable; commit `909d0a4b` classifies disk-full, I/O, corruption, and not-a-database failures as unhealthy and fails the writer closed. `ArkService::shutdown(deadline)` is async and deadline-bound; `Drop` closes ingress without joining indefinitely. These claims still need recovery-focused integration proof.

KOS-428 acceptance requires an actual existing peer in a temporary-DB restore test, preservation of untransmitted local changes, no reused origin sequence, no Ready service after disk-full/commit/panic/restore failure, and a stated OS/power-loss durability boundary. A safe contract must distinguish expected user rollback from recovery-required replay and must never silently rotate stable device identity.

## Arena rubric

1. Prevent identity reuse after restoring an older valid DB, including peer-visible resnapshot behavior.
2. Preserve every committed but not-yet-transmitted local change, or reject a restore before mutating the live DB when preservation cannot be proven.
3. Keep the stable device identity and existing-peer relationship; no silent key/device replacement.
4. Make crash/retry/partial-restore behavior idempotent and fail closed.
5. Keep the wire and persistence contract as small and backward-compatible as acceptance permits; spell out compatibility with old peers.
6. Prove behavior with production SQLite and two persistent peers rather than mocks.

## Synthesis

Candidate B is the base: generate a fresh random incarnation into the staged SQLite image, and conservatively refuse restore before live mutation whenever live has a later committed feed head than the backup. It uses the existing rollback capsule/intent; an un-published incarnation cannot escape because sync is fenced.

Implemented so far: `watermark-v1` is advanced in the same transaction as each feed entry, bootstrapped from retained rows and SQLite's AUTOINCREMENT sequence, and retained when feed rows are pruned. Restore compares the staged and live watermarks before identity recovery or journal/apply, and returns a clear refusal with the live DB untouched when the snapshot is stale. This is deliberately conservative: it preserves local committed changes but does not yet provide incarnation negotiation or allow a stale restore with peer-safe replay.

Grafts from candidate A: carry forward the live HLC floor so new writes remain ordered against values peers may already have observed; retain monotonic source sequence floors where the source can be identified. Do not copy the entire version vector, because it could claim data absent from the restored snapshot. Feed comparison must use an explicit durable monotonic head, not `MAX(position)` over retained rows. Legacy peers fail closed after recovery. Full resnapshot remains disabled until a real persistent-peer test proves conflict and tombstone behavior.

The judge confirmed B's incarnation is safe when stored in staged SQLite: before the durable restore completion, startup rolls back to the old capsule and no network has observed the candidate. The judge flagged `MAX(position)` as non-monotonic under deletion and rejected blindly carrying the whole version vector. These findings are reflected in `docs/ark-exocortex.md` §9 and in this section. Candidate and judge files: `/tmp/kos428-architecture/candidate-a.md`, `/tmp/kos428-architecture/candidate-b.md`, `/tmp/kos428-architecture/judge.md`.
