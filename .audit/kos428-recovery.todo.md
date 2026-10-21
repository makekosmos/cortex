# KOS-428 recovery design and delivery

- [x] Ground the current SQLite, actor, snapshot, identity, and shutdown paths.
- [x] Frame two structurally distinct recovery-epoch designs and grade them against explicit safety criteria.
- [x] Cross-judge both candidates and select one design.
- [x] Select the conservative equivalent to recovery epochs: refuse a restore before live mutation whenever its committed-feed watermark is stale or cannot be proven.
- [x] Preserve unsent local changes or fail restoration before replacing the live DB.
- [x] Persist a monotonic committed-feed watermark and refuse stale restore before touching live data.
- [x] Verify a stale restore cannot drop a pending local usage event or reuse its origin sequence, then reconnect an existing Iroh peer and prove pending object changes resume (commit `7e4913fe`).
- [x] Inject a deferred SQLite constraint failure at the production actor's COMMIT boundary and verify all transaction effects and receipts roll back and the actor leaves Ready.
- [x] Verify stale-restore refusal and preserved pending edits with two persistent SQLite peers (`7e4913fe` and current restore integration tests).
- [x] Fail both production panic boundaries closed, preserve the active operation's unknown outcome, cancel queued work, and verify explicit owner/service restart.
- [x] Complete production SQLite failure injection for commit failure, unresolved restore recovery, DB actor panic, and ArkService request panic; prove the service no longer accepts work as Ready.
- [x] Complete supported-platform durability evidence. Exact-SHA CI run `39570134543` passed on macOS and Windows; both production SQLite artifacts verified WAL + `synchronous=FULL`, five trials of 700 checked 512-byte commits, and SQLite 3.46.0.
- [x] Reconcile the final contract, Linear acceptance, tests, and commits. KOS-428 and KOS-412 are Done with final evidence comments; see run `39570134543` and commit `a03daba3`.

## Grounding

Current SQLite is opened with WAL and `synchronous=FULL`; ADR-001 documents that successful commits still depend on OS/filesystem/hardware flush behavior. The backup API creates a consistent snapshot and a manifest validates size, database hash, schema identity, and SQLite integrity. Restore is an actor maintenance operation with a rollback capsule and restart of sync after success. The capsule repairs an interrupted restore; it does not protect sync identity from restoring an older valid snapshot.

The identity values that can roll back with a DB snapshot include HLC high-watermark, origin/change sequence, version vectors, usage sequence, committed-feed positions, and operation receipts. The stable iroh secret/device identity is preserved in normal snapshot restore. A peer can therefore already know a change identity which the restored DB may generate again. No recovery epoch is exchanged by peers today.

The actor supports explicit Ready/Failed lifecycle; startup recovery runs before the actor enters Ready. SQLite busy remains retryable; commit `909d0a4b` classifies disk-full, I/O, corruption, and not-a-database failures as unhealthy and fails the writer closed. Writer-task panic now moves the actor to `Failed`; the service-level request worker also stops after a handler panic. Neither owner returns to Ready. The active panicking request remains outcome-unknown. `ArkService::shutdown(deadline)` is async and deadline-bound; `Drop` closes ingress without joining indefinitely.

The opened production connection sets WAL, `synchronous=FULL`, foreign keys, and a five-second busy timeout in `db/open.rs`; the pinned Rust adapter resolves to `rusqlite 0.32.1` and `libsqlite3-sys 0.30.1`. Six focused production tests passed on macOS for commit failure, restore recovery failure, actor panic, request-worker panic, stale restore with pending local changes, and stale restore after an existing Iroh peer observed a change. Final exact-SHA CI also passed the full Engine suites on macOS (2,079) and Windows (2,232), including recovery paths. Both platforms uploaded production SQLite experiment artifacts; OS/filesystem/hardware flush behavior and physical power-loss guarantees remain explicitly outside the measured evidence.

Final artifact samples: macOS median average commit 283.356 µs (range 148.905–296.828 µs); Windows median average commit 2426.806 µs (range 2353.616–2541.518 µs). These are independent runner measurements, not an SLO or a cross-hardware comparison.

KOS-428 acceptance requires an actual existing peer in a temporary-DB restore test, preservation of untransmitted local changes, no reused origin sequence, no Ready service after disk-full/commit/panic/restore failure, and a stated OS/power-loss durability boundary. A safe contract must distinguish expected user rollback from recovery-required replay and must never silently rotate stable device identity.

## Arena rubric

1. Prevent identity reuse after restoring an older valid DB, including peer-visible resnapshot behavior.
2. Preserve every committed but not-yet-transmitted local change, or reject a restore before mutating the live DB when preservation cannot be proven.
3. Keep the stable device identity and existing-peer relationship; no silent key/device replacement.
4. Make crash/retry/partial-restore behavior idempotent and fail closed.
5. Keep the wire and persistence contract as small and backward-compatible as acceptance permits; spell out compatibility with old peers.
6. Prove behavior with production SQLite and two persistent peers rather than mocks.

## Synthesis

The reviewed incarnation design is deferred. The accepted recovery equivalent is stricter: refuse stale or unverifiable snapshots before live mutation. A new epoch would separate future event identities but would not recover local commits missing from the snapshot, so it is not sufficient to permit rollback safely.

Implemented so far: `watermark-v1` is advanced in the same transaction as each feed entry, bootstrapped from retained rows and SQLite's AUTOINCREMENT sequence, and retained when feed rows are pruned. Restore compares the staged and live watermarks before identity recovery or journal/apply, and returns a clear refusal with the live DB untouched when the snapshot is stale. Two persistent Iroh-backed SQLite peers prove peer-visible and pending local changes remain intact, later origin sequence increases, and pending objects resume after reconnect.

The explicit durable feed head is used instead of `MAX(position)` over retained rows. Stable registry identity and its signed sequence are carried forward on an accepted restore. The implementation does not copy the entire version vector, because it could claim data absent from the restored snapshot. Full resnapshot remains disabled until a real persistent-peer test proves conflict and tombstone behavior.

The judge flagged `MAX(position)` as non-monotonic under deletion and rejected blindly carrying the whole version vector. The final choice avoids publishing a new incarnation until restore can safely replay or merge missing data. These findings are reflected in `docs/ark-exocortex.md` §9 and in this section. Candidate and judge files: `/tmp/kos428-architecture/candidate-a.md`, `/tmp/kos428-architecture/candidate-b.md`, `/tmp/kos428-architecture/judge.md`.
