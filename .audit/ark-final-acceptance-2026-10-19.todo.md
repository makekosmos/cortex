# ARK final acceptance run (2026-10-19)

- [ ] Read the Principles section of pstack:poteto-mode.
- [ ] Phase A — Frame: define falsifiable completion predicate, scope, quantified effort, and blockers from current worktree, GitHub, and Linear.
- [ ] Phase B — Design the workflow: create the baseline verification harness, identify separable work, and record the reviewed phase sequence.
- [ ] Phase C — Run the loop: reproduce Stage 1 Windows failure; fix and verify; finish remaining boundaries; close structural review findings; verify recovery behavior; complete each logical commit.
- [ ] Phase D — Keep the audit trail: append evidence-backed decisions and cross-review it.
- [ ] Phase E — Verify and hand back: run full checks and desktop/mobile matrix on final SHA, independent final review, reconcile Linear acceptance, and prove all plan predicates.

## Work items from /tmp/cortex-thermo-ark-exocortex/05_remaining_plan.md

- [x] Reproduce `signed_hpke_replication_reaches_offline_provider_and_rejects_stale_inputs` on Windows and identify the transition that loses the route. Historical run `39081298180` failed waiting for consent after the recipient was online; Iroh connection and Hello succeeded.
- [x] Verify Iroh endpoint/ticket lifecycle, admission, and route readiness; test the send precondition rather than adding fixed waits. The fixture pre-seeded the live transport key before pairing, so its expected consent request was invalid.
- [x] Fix the demonstrated root cause, commit it, and get the Windows scenario green. Ancestor `68d7255b` removed the invalid consent wait and waited for admitted peer link; target passed Windows run `39084331701`, all-job Windows run `39087076119` passed at `f7fd1709`, and later full matrix `39136020199` passed at `42f93c2c`.
- [ ] Reconcile current code with the remaining ownership plan: domain/application/store/sync/net and ArkCore composition; one SQLite transaction boundary; remove obsolete internal facades.
- [ ] Verify `ark-store` exercises storage scenarios without `ark-core`, `ark-net` has no storage dependency, and desktop/mobile Cargo graphs match the whitelist.
- [x] Audit all `partN` source paths including tests; remove baseline entries only when paths are genuinely decomposed. Current `rg --files` inventory contains no `partN` source paths, and `node --test scripts/check-source-size.test.mjs` passes 11/11; the checker passes with 35 unrelated grandfathered >500-line files.
- [ ] Audit sync ports for required methods, typed outcomes at boundaries, and selective-sync narrowing/widening behavior.
- [x] Map every KOS-428 criterion to a concrete SQLite failure/recovery test and supported-platform evidence. On this tree `production_actor` passes 21/21, covering SQLITE_FULL, deferred commit failure, restore rollback/recovery-required, panic, reopen and bounded shutdown; two Iroh-peer stale-restore tests pass 2/2 and assert preserved local data/sequence plus peer resync. SQLite opens WAL+FULL and rechecks it on reopen. Windows/macOS matrix evidence exists at ancestor CI SHA `42f93c2c`; physical power-loss behavior is explicitly not claimed.
- [ ] Update architecture/build/recovery docs to match measured guarantees.
- [ ] Run full local checks and final desktop/mobile CI on the exact final SHA.
- [ ] Update KOS-412, KOS-424, KOS-425, KOS-428 and related Linear records only with verified evidence.

## Current evidence (2026-10-19 continuation)

- `cargo nextest run -p ark-store`: 30 passed, 0 skipped. This proves the crate's current unit suite runs independently; it does not prove the planned object/query/storage scenarios have all moved out of `ark-core`.
- `ark-core/Cargo.toml` still directly depends on `rusqlite`; source inventory finds SQLite use across canonical migrations, db command handlers, and tests. Storage ownership is therefore still incomplete.
- `cargo nextest run -p ark-sync`: 102/102 passed after confirming the current direct-client loop and protocol progress live in ArkSync.
- `cargo nextest run -p ark-core production_actor --test-threads 1`: 21/21 passed, including SQLite FULL, commit failure, restore failure, recovery-required, panic, reopen, and bounded shutdown.
- `cargo nextest run -p ark-core stale_restore --test-threads 1`: 2/2 passed with two existing Iroh peers, including preservation of pending local data and origin sequence followed by resync.
- Commit `3920db87` documents the configured SQLite durability and the limits of its evidence in `docs/ark-core.md`.
- Commit `7680d34f` moved pending-object and type-version schema migration into `ark-store`, removed the Core migration facade, and retained the enclosing initialization savepoint. Store migration/schema tests (31/31), Core init migration test (1/1), type-registry tests (14/14), and `clippy -p ark-store -p ark-core --all-targets -D warnings` passed.
- Storage extraction remains incomplete: `ark-core` still uses rusqlite in object backends, sync/task merge, usage, replication policy, and canonical-type migration paths; the facade/dependency checklist remains open.

- [x] Move durable sync-tombstone CRUD and projection out of `ark-core::db` into `ark-store::sync_tombstones`; remove the old Core facade and migrate production and integration-test callers. Commit `671465c3`.
  - `cargo nextest run -p ark-store tombstone_updates_are_monotonic_and_keep_known_type_identity`: 1/1 passed.
  - `cargo nextest run -p ark-core -E 'test(tombstone)'`: 9/9 passed.
  - `cargo clippy -p ark-core -p ark-store --all-targets -- -D warnings`: passed.
  - `pnpm run test:ark-db-writers`: 83 path/category counts match.
  - Desktop (`x86_64-apple-darwin`) and mobile (`aarch64-apple-ios`) dependency-graph gates passed.
  - The full ownership/dependency item remains open: Core still directly depends on `rusqlite`, and SQLite-facing code remains across command, migration, sync, and replication paths.
- [x] Move usage tracked-app/session/event/day CRUD and usage history reads from Core SQL facades into `ark-store`; preserve the same connection/transaction boundary and migrate Core callers. Commits `0704143e`, `84697770`, `2838b613`.
  - `cargo nextest run -p ark-core -E 'test(usage)'`: 28/28 passed after CRUD and again after history-read extraction.
  - `cargo clippy -p ark-core -p ark-store --all-targets -- -D warnings`: passed after each extraction wave.
  - `cargo nextest run -p ark-store`: 67/67 passed independently of `ark-core` after CRUD extraction.
  - `pnpm run test:ark-db-writers`: 83 path/category counts match after the CRUD move.
  - Remaining usage SQL in Core is analytics, process/game-playtime queries, span projection, and the usage sequence journal. This does not close the overall Store ownership criterion.
- [x] Move usage sync-journal writes, origin lookup, revision checks, and receipt progression out of Core into the Store-owned usage-sync log module. Commit `675dec70`.
  - `cargo nextest run -p ark-store`: 67/67 passed independently.
  - `cargo nextest run -p ark-core -E 'test(compaction) | test(cursor) | test(usage_sync_log)'`: 16/16 passed.
  - The full usage filter passed 28/28; Clippy for Core and Store passed with `-D warnings`.
  - `pnpm run test:ark-db-writers`: 82 path/category counts match.
- [x] Move todo/project/tag row reads out of Core into `ark-store::legacy_entities`. Commit `50a3834a`.
  - `cargo nextest run -p ark-core -E 'test(load_all) | test(usage)'`: 32/32 passed.
  - `cargo nextest run -p ark-store`: 67/67 passed; `cargo check -p ark-core -p ark-store --all-targets` and Clippy passed.
  - The full ownership criterion remains open: canonical migrations, object/sync commands, usage analytics, replication policy, and Core command handlers still use rusqlite.
- [x] Move usage analytics, process search, game playtime aggregation, span projection, and title totals into `ark-store::usage_queries`, removing the Core usage module and its export facade. Commit `8063dfb9`.
  - `cargo nextest run -p ark-core -E 'test(usage)'`: 28/28 passed after the move.
  - `cargo nextest run -p ark-store`: 67/67 passed; Store builds and tests without `ark-core`.
  - `cargo clippy -p ark-core -p ark-store --all-targets -- -D warnings` and source-size/format checks passed.
  - `pnpm run test:ark-db-writers`: still 82 path/category counts, with no untracked SQL writers.
- Revalidated on `8063dfb98d7b3a31d5dc8472d658c1514904e341`: `cargo nextest run -p ark-store` 67/67, `cargo clippy -p ark-core -p ark-store --all-targets -- -D warnings`, desktop/mobile dependency graph gates, SQLite writer census (82 counts), format and source-size checks all pass.
- Current GitHub state: branch is 104 commits ahead of `origin/codex/ark-stage2-boundaries`; no PR is open and the latest available full CI is historical run `39136020199` on ancestor `42f93c2c`, not the current head. Exact-head CI remains open.
- Current Linear state: parent KOS-412 and child KOS-424/425/428 are `Done`, while the branch still fails their observable ownership/final-matrix criteria. The whole child set KOS-417, 420-429 currently reports Done; statuses must be reconciled after the acceptance evidence is complete, not treated as proof of completion.
- The read-only owner inventory is recorded at `/tmp/ark-store-census-findings.md`; it recommends committed-feed persistence, device-registry adapters, and pending-object storage as the next verifiable extraction units. No agent-authored code was accepted.
- Independent review follow-up: the reported task-merge HLC mismatch is not present in the current checkout. `mutation.rs` uses the persisted legacy revision for an existing legacy object, Store does not replace `bootstrap.source_hlc`, and `deleting_legacy_task_uses_the_same_bootstrap_conflict_set_it_persists` plus `editing_legacy_task_builds_mutation_from_the_new_bootstrap_baseline` pass (2/2). The reported `append_bootstrap_with_revision`/`record_revision` branch is absent; the rollback integration test already injects the journal write failure after bootstrap projection and checks the related state. No code change was warranted for those stale findings.
- [x] Move inbox receipt SQL, digest identity checks, capacity accounting, durable state parsing, and payload release into `ark-store::committed_feed`, retaining the wire-size check at the Core boundary; remove Core's duplicate SQL. The SQLite writer census now records 81 path/category counts (one fewer Core writer path, two additional Store mutations). Store inbox behavior test 1/1, Core inbox and durable-replay tests 6/6, Clippy, formatting, and source-size gate passed.
- [x] Move outbound snapshot/cursor/receipt/applied persistence and the feed snapshot watermark read into `ark-store::committed_feed`; keep the Core policy-identity check at the boundary and route transactional progress commands directly to Store. Store committed-feed tests 4/4, Core committed-feed/feed integration tests 19/19, Clippy, writer census (81 counts), formatting, and source-size checks passed. No feed/protocol behavior changed.
- [x] Move committed-feed snapshot metadata, log-page selection, delivery projection, and policy-bound page reads to Store. Core feed commands call Store directly and supply the wire byte ceiling at the protocol boundary; remove the duplicate Core SQL and wrapper modules. Store read-snapshot projection and feed-log race tests 2/2, Store committed-feed tests 6/6, Core committed-feed tests 15/15, Clippy, format/source-size gates, and SQLite writer census (81 counts) passed. The log-page race test failed before the savepoint fix with an empty page after concurrent pruning, then passed with the full check-and-select sequence pinned to one SQLite snapshot.
