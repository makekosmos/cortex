# Object-mutation cutover sketch

## Architect

- [x] Ground: trace the four object-mutation commands, Core application session, Store persistence helpers, and accepted SQLite ownership design.
- [x] Sketch: produce two structurally distinct API and module designs with concrete call sites.
- [x] Agree: compare both designs against ownership, atomicity, compatibility, and final Core SQLite removal; proceed without a human checkpoint.
- [ ] Implement: migrate the selected production path and delete obsolete callers and adapters in the same unit.
- [x] Extract pure canonical object-write validation to `ark-domain`; keep Core's database-backed type lookup as a temporary boundary adapter until the Store command cutover.
- [x] Verify the extraction with `cargo check -p ark-domain -p ark-core --all-targets` and the canonical task-write plus Agenda contract tests (5/5).
- [x] Move persisted type resolution and the canonical ingress entry point to `ark-store`; migrate Core adapters and tests, then delete the Core ingress module.
- [x] Verify the Store-owned ingress with `cargo check -p ark-domain -p ark-store -p ark-core --all-targets` and the canonical task, object-revision, and Agenda contract tests (16/16).
- [x] Move object upsert/delete request and commit types to `ark-application`; preserve Core's existing internal paths as re-exports during command migration.
- [x] Verify the contract move with `cargo check -p ark-application -p ark-core --all-targets` and the canonical task, object-revision, and Agenda contract tests (16/16).
- [x] Move local upsert type and stale-snapshot decisions into a pure Application function; verify both outcomes with focused Application tests and existing Core regressions.
- [ ] Scrap: if implementation requires repeated Core/Store ownership workarounds, re-ground and replace the sketch instead of patching around it.

## Arena

- [x] Frame: define the object-mutation API sketch as the artifact and gradeable criteria.
- [x] Fan out: obtain two independent candidates with separate output files.
- [x] Cross-judge: have an independent reviewer score both candidates against the same criteria.
- [x] Pick: score each candidate criterion by criterion and select a base.
- [x] Graft: carry forward only concrete strengths that do not weaken the selected boundary.
- [x] Verify: reconcile the selected shape against source ownership, compile constraints, and existing transaction tests. This verifies the design only; implementation verification remains open.

## Rubric

- Removes Core-owned SQL/session adapters without adding a compatibility layer scheduled for deletion.
- Keeps each command's existing single immediate transaction and atomic revision, feed, and tombstone effects.
- Keeps canonical validation, sync policy, and conflict decisions outside Store while giving Store complete persistence operations.
- Uses types and crate dependencies that compile without an `ark-core` dependency cycle.
- Preserves Engine request/response and sync wire behavior with user-visible regression tests.
