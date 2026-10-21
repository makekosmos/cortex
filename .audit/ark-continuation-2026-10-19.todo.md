# ARK completion continuation

## Figure-it-out workflow

- [x] Read the Principles section of the pstack:poteto-mode skill.
- [x] Phase A — Frame. The done predicate is every item in `/tmp/cortex-thermo-ark-exocortex/05_remaining_plan.md` plus KOS-412/424/425/428 acceptance, proven on the final artifact and exact CI SHA. Scope is the remaining ARK storage/sync boundaries, structural gates, restore evidence, docs, and Linear reconciliation. Current evidence shows roughly 11–20 hours of code and verification remain, plus CI queue time. Windows/macOS CI is still only proven at ancestor `42f93c2c`; the current branch is 155 commits ahead of that SHA.
- [x] Phase B — Design the workflow. Work in verifiable units: close structural gates and policy behavior tests, close the KOS-428 compatibility gap, complete one Store ownership slice at a time, update docs only from measured evidence, then run final local and exact-SHA cross-platform acceptance. Keep the current single-DB and caller transaction boundaries. Do not edit pre-existing `.audit` records.
- [x] Phase C — Run the loop. The Windows regression, source-name gate, migration-to-restore fixture, selective-sync actor test, transport capability split, legacy task migration, and task mutation adapter are tested and committed. Continue one ownership transfer at a time.
- [x] Phase D — Keep the audit trail. The paired TSV records the evidence-backed changes and the transport design decision.
- [ ] Phase E — Verify and hand back. Check all plan predicates against current source, tests, exact-SHA CI, independent review, and Linear.

## Remaining plan items

- [x] Phase 1, Windows regression fix. The invalid consent wait was removed after the test had already paired/established transport. The Windows target and full desktop/mobile jobs passed at `f7fd1709` in run `39087076119`.
- [x] Close structural source-name gate. Commit `c3c325da` removes the empty part-name baseline and makes any `partN` filename or directory segment fail; 10 focused tests and the full source-size gate pass.
- [x] Add KOS-428 migration-to-restore compatibility proof. Commit `92527056` starts from a legacy task database, runs startup migration, publishes a manifest-verified snapshot, then restores the migrated task; focused test passes. This preserves the documented rule that restore rejects an older structural schema.
- [x] Verify policy exclusion, widening backfill, and shrink retention through file-backed Iroh actors. Commit `406972fb`; focused nextest passes. Policy rules use canonical type IDs (`com.kosmos.note`); `note_obj` is a legacy alias.
- [x] Close sync-port capability and typed-outcome findings. Accepted design: `.audit/ark-transport-capabilities.design.md`; implementation is in commit `0976f016`, with 757/757 ark-core/ark-net/ark-sync tests passing and the pre-commit gate passing.
- [x] Move the legacy task rewrite lifecycle out of Core. `ark-domain` owns pure task mapping; `ark-store` owns loading, backup and the single update/version transaction; actor startup calls the Store operation. Focused migration, actor cutover and migration-to-restore tests pass; all 90 `ark-store` tests pass.
- [x] Move the task mutation SQLite adapter and local causal commit algorithm into Store. Core keeps command admission and actor scheduling, and passes the existing immediate transaction through the Application session contract. The Core adapter and duplicate local commit implementation were deleted.
- [x] Move local object delete, local object upsert, and book-cover persistence to Store-owned immediate transactions. Core keeps actor admission and settlement; Application owns the book-cover plan. Focused Application, Core RPC, facade, and Store tests pass.
- [ ] Document supported restore behavior; keep physical power-loss limits explicit.
- [ ] Finish remaining Core/Store/Sync/Net ownership boundaries while preserving one SQLite transaction and stable Engine/wire contracts.
- [ ] Reconcile and update architecture/build/recovery documentation from actual final behavior.
- [ ] Run the full local gate and exact-final-SHA desktop/mobile/OS matrix; resolve all failures.
- [ ] Independent final review; fix confirmed findings and reverify.
- [ ] Update KOS-412, KOS-424, KOS-425, KOS-428 only after their acceptance evidence is complete.
