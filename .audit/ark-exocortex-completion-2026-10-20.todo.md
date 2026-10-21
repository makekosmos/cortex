# ARK exocortex completion run

## Definition of done

Complete every phase in `/tmp/cortex-thermo-ark-exocortex/05_remaining_plan.md`. The storage boundary passes `scripts/check-ark-core-storage-boundary.mjs`; the dependency graph matches the accepted architecture; compatibility, sync-policy, recovery, and wire behavior pass on the final commit; the required platform CI is green on that exact SHA; architecture docs and Linear acceptance records match the verified result.

## Run

### Sync-policy write boundary design

- [x] Frame: define the output artifact, the behavior and ownership rubric, and two structurally distinct write-boundary candidates.
- [x] Fan out: ask two GPT-6-Luna candidates for read-only designs with separate output files.
- [x] Cross-judge: use an independent model to score both candidates against the rubric.
- [x] Pick: choose the named Store writer task, retaining Core admission and authorization while avoiding a generic second dispatcher.
- [x] Graft: use the closed two-operation task shape and preserve explicit operation-specific result types.
- [x] Verify: move both writes to Store-owned immediate transactions, retain the profile savepoint, and test rollback and sync-policy behavior.

- [x] Read the remaining plan and verify current branch, recent commits, worktree, and existing acceptance evidence.
- [x] Capture the current storage-boundary baseline and distinguish production from test-only references.
- [x] Fix the version-vector read/merge/write race with a Store transaction and retain exact replacement semantics.
- [x] Move policy-bound snapshot and feed reads behind Store-owned read capabilities.
- [x] Move task conflict-set reads behind a Store read capability and keep the serialized Engine response shape in an Application-owned result type.
- [x] Move each remaining SQLite operation family to Store-owned typed capabilities, migrating callers and deleting Core SQL paths in the same unit.
- [x] Remove Core SQLite source and Cargo dependencies, including test fixtures, and make the boundary gate pass.
- [ ] Complete the KOS-428 recovery matrix on supported platforms and map each acceptance criterion to evidence.
- [ ] Run the full local checks and desktop, mobile, and Windows CI on the final SHA.
- [x] Run the independent correctness and thermo-nuclear code-quality reviews; resolve confirmed findings.
- [ ] Update architecture and recovery documentation, reconcile KOS-424, KOS-425, KOS-428, and KOS-412 in Linear, and record final evidence.

## Baseline

On 2026-10-20, `scripts/check-ark-core-storage-boundary.mjs` reported 846 findings: 162 production findings, 672 test-file findings, 10 inline-test findings, and 2 debug-only findings. This is a source scan, so each finding must be confirmed against the operation map before it is treated as a distinct migration unit.

The plan's earlier estimate of 15–25 active hours is stale. The current baseline contains 846 source findings and only local recovery evidence; re-estimate after the production operation families and test fixture moves are mapped.

## Current verified commits

Final local acceptance on `codex/ark-stage2-boundaries`: Core has no SQLite dependency or SQL source, the storage census reports zero SQLite ownership references, and the full affected gate passed with 2,079 Rust tests (5 skipped) plus Manager 124/124. Independent correctness/security and maintainability reviews found no actionable blockers. Exact-SHA CI remains the platform acceptance gate; Linear completion follows that result.

- `d321caf5` makes version-vector merges atomic inside the Store writer transaction. The four affected packages passed 836 tests, targeted stale-policy and concurrent-merge tests passed, Clippy passed, and the repository pre-commit checks passed.
- `e9e3a9d9` moves policy-bound snapshot and feed reads to Store-owned read capabilities. `ark-core` and `ark-store` passed 716 tests, Clippy passed, desktop and mobile graph checks passed, and the repository pre-commit checks passed.
- Task-merge conflict reads now use the Store snapshot capability; the response DTO lives in Application rather than Store. The 54 filtered Core and Store task-merge tests passed, Clippy and formatting passed, and the existing service test checks the camelCase conflict response.
- Device-registry sync admission reads now use Store sync capabilities; Core still authenticates the peer before reading. All 10 filtered device-registry tests and Clippy for `ark-core`/`ark-store` passed. The boundary scanner remains at 845 findings and 161 production findings because adjacent registry command helpers still expose SQLite in the same family.
- Device-registry snapshot and event-page reads now use a named Store capability. Core retains Engine authorization, sync-principal checks, pagination bounds, and response shaping. All 10 filtered device-registry tests and Clippy passed; the source-size check passed. The boundary scanner still reports 845 findings and 161 production findings, so remaining transaction commands in the shared module continue to dominate its count.
- Local, accepted-peer, and sync-session profile reads now use Store sync capabilities. Core still validates the Engine or authenticated peer principal. Eight sync-policy tests, Clippy, and formatting passed. The boundary count remains 845/161 because the same Core source file still contains profile write commands and their transaction types.
- Registry handshake verification now loads its device registry through the Store capability while Core retains signature verification and peer-principal validation. The filtered handshake test and Clippy passed. The boundary scan remains 845/161 because the same file still contains the signing command and its writer transaction.
- Local-profile and accepted-peer policy writes now use named Store writer methods through one closed actor task. Core keeps Engine and peer-principal checks, class/weight admission, cancellation, and reply handling; Store starts the immediate transaction and keeps profile savepoint handling. Savepoint rollback failures now retain both the operation and cleanup errors. Seven filtered Core sync-policy tests, the Store late-write rollback test, the persisted-profile integration test, and Clippy for `ark-core`/`ark-store` passed. The source-size check passed. The boundary scanner now reports 845 findings, including 160 production findings; remaining families are not complete.
- Usage sync-log compaction now runs through a Store-owned immediate transaction and named writer task; the Engine maintenance class and admission remain in Core. Its bounded maintenance test and Clippy for `ark-core`/`ark-store` passed. The boundary scanner now reports 844 findings: 158 production, 672 test-file, 10 inline-test, and 4 debug-only.
- Committed-feed progress now uses an Application-owned `OutboundProgressWrite` and one Store-owned immediate transaction. Store checks the full accepted `PolicyIdentity` in the same transaction as the progress mutation; Core retains peer authorization, SyncLive admission, cancellation and actor reply behavior. Removed the duplicate Core operation enum and the Core SQL helper implementations. The 16 committed-feed Core tests, actor peer-authorization test, and Store writer transition test passed; Clippy with `-D warnings`, formatting, and the source-size gate passed. The source scan now reports 844 findings: 157 production, 672 test-file, 10 inline-test, and 5 debug-only; the overall gate still fails.
- Local task-merge append now uses an Application result type and a named Store writer operation; Core retains Engine authorization, interactive admission, cancellation, and actor reply scheduling. Store keeps replay detection, conflict digest checks, journal persistence, materialization, and the final conflict read in one immediate transaction. The remote inbound append path is unchanged. Store task-merge tests passed 23/23; Core task-merge tests passed 34/34, including principal rejection, the debug test backend, and authenticated wire delivery. Clippy for `ark-core`, `ark-store`, and `ark-application`, formatting, source-size, and format checks passed. Release compilation exposed test-only helper imports in two sibling Core modules; those imports are now debug-gated, and `cargo check -p ark-core --release` passes without warnings. Desktop Windows, desktop Apple, and mobile Android dependency graph checks passed. The boundary gate still fails at 844 findings because the remaining remote task-merge and other SQLite families still exist.

Inbound inbox receipts now use an Application-owned write request and a named Store writer transaction. Core validates the authenticated peer context; Store checks policy identity, registry admission, removed-peer state, payload size, and receipt idempotency under one `BEGIN IMMEDIATE`. Application owns the batch-size constant shared by `ark-sync` and Store. Full Core and Store tests passed: 432 and 106 respectively. Clippy, release compilation without warnings, formatting, source-size, and format checks passed. The boundary scan reports 845 findings: 154 production, 676 test-file, 10 inline-test, and 5 debug-only. Production findings decreased by one; test-file findings increased by two. The boundary gate remains open.

Remote task-merge inbound batches now run through a typed actor task and one Store-owned immediate transaction. Store rechecks the local policy identity, Full/unfiltered task requirement, registry admission, and removed-peer status before applying bootstrap/operation entries and recording the replay receipt/cursor. Core keeps authenticated-peer matching, SyncLive admission, and error/wire mapping. Added a later-entry collision test proving that earlier operation/journal writes and progress all roll back together; existing bootstrap ordering, replay, convergence, and excluded-policy tests still pass. Extracted the shared task-policy predicate into Application and removed transaction-taking helpers from the public Store API. A strict quality review found three issues (discarded query, public raw-transaction helpers, duplicated predicate); all three were fixed. Final validation passed: release check, Clippy with `-D warnings`, formatting, all 558 Application/Core/Store unit tests, source-size gate (35 pre-existing >500-line files), diff check, and desktop Windows/macOS plus mobile Android graph checks. The boundary scanner now reports 830 findings: 136 production, 678 test-file, 10 inline-test, and 6 debug-only. Commit `f02d8ac3` contains this batch.

## Latest boundary scan

The source scanner currently reports 422 findings: one production manifest dependency, 363 test-file findings, 49 inline-test findings, and 9 debug-only findings. Production SQL/type/lifecycle references have been removed; the remaining production finding is `rusqlite` in Core's manifest. The gate still fails on the dependency and test-only references.

## Legacy object revision batch

Core legacy write and receive facades now use Store's existing object revision read/write capabilities instead of embedding SQL against `object_sync_versions`. Core retains HLC comparison and mapping policy; writes remain in the caller's transaction. Targeted legacy write and revision tests passed (12/12). The full App/Core/Store unit run passed 557/558; an unrelated Iroh device-rename test timed out waiting for a remote event, then passed when rerun alone. Clippy, release check, formatting, and source-size checks passed. Commit `a6dfebe9` contains this batch.

## Test-only module classification batch

The storage-boundary scanner now resolves external Rust module files declared under `#[cfg(test)]`, including nested modules, and classifies their findings as test-file rather than production. Its focused test covers both test-only and production module declarations. All 5 scanner tests pass. This corrects a 21-finding misclassification; it does not reduce actual findings or close the gate. Commit `27e171f3` contains this batch.

Next production family identified: configuration, authorized-node, and grant upserts in `db/integration_replication_records.rs` and `db/integration_replication_grants.rs`. They currently keep HLC/epoch policy in Core and call a savepoint-backed vector writer. The App integration-admin capability is not a drop-in replacement because its transition rules differ. Preserve validation and rollback as one unit when migrating; do not route through the admin capability without reconciling those contracts.

## Recovery baseline recheck

The earlier HPKE CI failure was on run `39081298180` at `2bbca173`: the fixture had already installed peer authorization, so the recipient became online without an incoming pairing request while the test waited for consent. Commit `68d7255b` changed the fixture to wait for authenticated peer presence; Windows passed the test and full CI at `f7fd1709` (`39087076119`). The later full CI on this branch passed Windows, macOS, and Android at `42f93c2c` (`39136020199`). This closes the historical failure, but neither run validates the current SHA `f02d8ac3` or its subsequent commits.

## Integration administration census

- [x] Frame a read-only two-slice census: admin writes and integration queries.
- [x] Fan out two GPT-6-Luna readers on disjoint slices.
- [x] Aggregate source evidence and use it to shape one cohesive App/Store cutover.
- [x] Report gaps and test evidence before implementation.

Both reports are in `/tmp/ark-integration-admin-{write,query}-census.md`. Writes need four typed operations with validation and their related entity/vector mutations in the Store's existing immediate transaction. Reads need one snapshot-bound Store capability shared by Engine-facing integration reads and SyncLive outbound validation; Core retains principal checks, request routing, runtime-space checks, and wire shaping. Existing RPC error and response shapes are compatibility constraints. The current direct Core transaction helpers and raw-connection `IntegrationReadQuery` are legacy paths to remove in the same cutover.

## Integration replication boundary batch

- [x] Frame one shared ownership rubric and inspect the current integration command, Store, and signed-frame paths.
- [x] Compare two structurally distinct transaction-boundary designs and get an independent judgment.
- [x] Select a typed Store write capability executed only after the existing weighted Core actor task is admitted; do not queue closures.
- [x] Move signed-frame transition validation to Application. Keep the replay callback result typed as `SignedSyncError`.
- [x] Move integration use-case decisions into Application and SQLite row codecs, vector updates, replay reservation, savepoint, and transactions into Store for the production and debug writer paths.
- [x] Keep Core's typed actor request, authenticated-peer extraction, identity error mapping, weighted actor admission, and service orchestration. Store rechecks registry admission inside the transaction.
- [x] Remove the production Core SQLite frame session. Keep the former adapter only under `cfg(test)` for legacy direct-session fixtures; those fixtures remain to migrate.
- [x] Run the integrated integration-replication behavior suite, crate checks, lint and local gates once for this family, then commit.

Signed-frame evidence so far: 5 Core signed-integration tests passed through the Store path; Core and Store targeted checks and Clippy with `-D warnings` pass. The full three-crate run completed before module-only file splits; a broader `check:affected` run is still executing. The initial source-size failure (files at 501/523/548 lines) was resolved by moving the signed-frame task, handler, and Store capability to named modules; the source-size gate now exits successfully.

Synthesis evidence: `/tmp/ark-integration-candidate-actor.md`, `/tmp/ark-integration-candidate-contract.md`, and `/tmp/ark-integration-design-judge.md`. The typed contract candidate was selected because it fits the existing Store `WriteCapabilities` model and avoids callbacks in queued actor messages. The independent judge found both candidates omitted a typed scope for applying the entity batch under replay savepoint; this batch must preserve it as one scoped Store operation with typed failure. The use-case module currently lives at `ark-core/src/application/signed_integration.rs`, so moving it into `ark-application` is part of the work, not a presumed existing boundary.

The signed-frame Application policy move covers identity verification, pre-existing trust, node and grant epoch transitions, lease and credential fences, HLC checks, entity ordering, duplicate keys, and version-vector decisions. The production SQLite session now lives in Store, while Core retains auth, actor admission, and orchestration. Integration admin writes and reads now follow the same Application/Store boundary with typed read capabilities. Final family verification passed: release check, Clippy with `-D warnings`, formatting, all 556 App/Core/Store unit tests, diff check, and `scripts/check-source-size.mjs` (35 grandfathered over-500-line files; no new violation). Commit `136f912f` contains this batch. The broader storage-boundary scan is down to 834 hits; exact-SHA CI is still pending.

## Integration revision write batch

Configuration, authorized-node, and grant revision policy now lives in `ark-domain`. Store owns the compatibility write functions, SQL persistence, savepoint rollback, and sync-version vector bump. Core keeps the existing public `db` function paths by re-exporting Store functions, and the old SQL-bearing Core implementations were removed. All 622 App/Core/Store/Domain library tests pass; Clippy with `-D warnings`, release compilation, formatting, source-size/pre-commit checks pass. Commits `0b1775d1` and `47ecf904` contain this batch and the test-only fixture classification.

The storage-boundary scanner now reports 819 findings, including 96 production and 707 test-file findings. Marking the remaining integration replication helper module `cfg(test)` preserves its fixtures and removes 8 false production findings; the 8 focused integration tests pass and the pre-commit checks pass. The gate remains open. The next architectural work is the Core command executor/transaction boundary; moving individual SQLite methods will not satisfy the target while `DbCommand` still receives `rusqlite::Transaction` from Core.

## Debug build check

Plain `cargo check -p ark-core` fails in the existing debug-only test backend because `ark_store::read_snapshot::test_support` is enabled only by `ark-store`'s dev-dependency feature. A broad `cfg(test)` conversion is invalid: integration-test builds link Core as a normal library and depend on the `cfg(debug_assertions)` test backend. That attempted conversion was fully reverted; the supported release check passes and all 622 library tests pass. If plain debug `cargo check` remains a required gate, resolve this through an explicit test-support feature/build-target design rather than changing individual cfgs.

## Inbound device registry writer batch

`ApplyRegistryEventsCommand` no longer receives a SQLite transaction. Core checks the authenticated sender, sync class, page limit, and admission weight before actor scheduling. Store now applies the event page, validates known-peer control access, updates projections, and computes digest/admission results inside one immediate transaction. Existing API result and `DbError` categories are preserved. Added a behavioral test where a conflicting second event rolls back the first event and projection change. All 623 App/Core/Store/Domain tests pass; the 11 registry tests, Clippy with `-D warnings`, release compilation, formatting, diff check, source-size check, and pre-commit passed. Commit `8e8cd21b` contains this batch. The scanner still reports 820 findings, including 93 production findings; the boundary gate remains open.

Next large cutover candidate: finish the device-registry write family, including local signed registry-event creation and paired-device enrollment. Keep key material and transaction-local sequence selection consistent, apply registry policy in Application, and commit the event/projection plus transport pin atomically in Store. This family is separate from the already completed inbound event-page writer and must not reintroduce `Transaction` into Core commands.

## Structural source-name gate verification

Verified the existing semantic-filename gate and its behavior tests: `node --test scripts/check-source-size.test.mjs` passed 10/10; repository search found no part-numbered Rust source files; `node scripts/check-source-size.mjs` passed with only the existing over-500-line grandfathered files. This closes the `partN` prohibition criterion. The storage ownership scanner remains a separate open gate at 820 total / 93 production findings.

At the current HEAD, the full four-crate suite passed 623/623, Clippy across all targets with `-D warnings`, release compilation, formatting, and the part-name/source-size gates. Collapsed the one-use registry-admission wrapper into the authenticated entity write, preserving the admission check in the same transaction as inbox persistence. The focused 11 registry tests and pre-commit passed. Commit `82f8f754` contains this simplification; the boundary scanner fell to 819 findings, 92 in production. The overall boundary gate remains open.

The boundary scanner had also mislabeled 27 SQLite references inside `#[cfg(test)]` imports and functions as production. Added item-range classification and a fixture that asserts both test-only and production code are detected under their correct scopes. All 6 scanner tests and pre-commit passed. Commit `d9b17c45` contains the fix. Current counts are 819 total, 65 production, 37 inline-test, 711 test-file, and 6 debug-only; the gate remains open.

## Outbound task-merge writer cutover

Moved outbound batch preparation and acknowledgement from Core SQLite commands into typed Store writer operations. Application owns delivery-policy validation and the wire-size batch builder; Core keeps peer authentication, sync-class checks, queue admission, actor scheduling, and public response behavior. Full crate tests passed (623/623) before the actor module was relocated; the focused task-merge suite passed again after relocation (35/35), release checks and Clippy passed, and source-size plus pre-commit gates passed. The no-comments review removed one now-unneeded comment and found no `MUST KILL` flags. Commit `0de9b02b` contains the cutover. The Core production SQLite census is now 57 references (47 type/import, 6 lifecycle, 3 SQL, 1 rusqlite dependency), with 811 total references; the boundary gate remains open.

## Legacy object revision candidate reconciled

The old checklist above duplicated the completed revision cutover in commit `a6dfebe9` (documented under “Legacy object revision batch”): Core compatibility ingress already uses Store-owned read/write capabilities, while HLC policy remains in Core/Application and writes remain inside the caller's transaction. The next boundary work is test ownership, not another revision API.

## Committed-feed test ownership batch

- [x] Move committed-feed persistence, progress, inbox, and selective-sync SQL tests from Core into Store-owned test modules.
- [x] Remove the test-only Core inbox and outbound-progress connection adapters; keep the sync transport size check covered by the Store writer test.
- [x] Verify `cargo nextest run --manifest-path core/crates/ark-store/Cargo.toml -p ark-store --locked` (125/125), `cargo nextest run --manifest-path core/crates/ark-core/Cargo.toml -p ark-core --lib --locked` (421/421), and Clippy for both crates.
- [x] Preserve the boundary checker without exclusions. At `5ec67251`, it reports 734 violations: 1 manifest dependency, 9 debug-only, 49 inline-test, and 675 test-file.

## Snapshot test ownership batch

- [x] Move snapshot restore contract tests and legacy startup migration coverage from Core into Store-owned unit-test modules.
- [x] Remove the Core snapshot test include and test-only imports; enable Store's test hooks under `cfg(test)` while preserving the explicit `test-support` feature path for external test-support consumers.
- [x] Verify all 410 ArkCore library tests and Store snapshot/full suites (11 focused, 136 total); Clippy for ArkCore and ArkStore with `-D warnings`, workspace formatting, and source-size checks pass.
- [x] Keep the ownership scanner unchanged and without exclusions. At `1ee090a7`, it reports 711 findings: 1 manifest dependency, 9 debug-only, 49 inline-test, and 652 test-file findings.

## Object-query and maintenance test ownership batch

- [x] Move the Core `time_entries_search_and_maintenance` persistence/query tests into ArkStore, including FTS rebuild/update/removal, alias queries, running-time queries, sync key-values, clear-all, and trashed-row deletion.
- [x] Keep their storage contract assertions and fixtures intact while using Store APIs directly; remove the Core test include and now-unused imports.
- [x] Verify Store's 12 moved tests (12/12), Core's remaining library tests (398/398), Clippy for Core and Store with `-D warnings`, workspace formatting, and source-size checks.
- [x] Keep the ownership scanner unchanged and without exclusions. At `2fab9b94`, it reports 701 findings: 1 manifest dependency, 9 debug-only, 49 inline-test, and 642 test-file findings.

Next: continue moving storage-owned tests and fault injection out of Core while retaining actor behavior tests at the Core boundary.

## Schema migration test ownership batch

- [x] Move Core's three database schema creation and migration behavior tests to ArkStore, where they directly exercise Store initialization and persisted data preservation.
- [x] Preserve legacy-row fixtures and migration assertions while deleting the Core test include.
- [x] Verify Store (151/151), Core (395/395), Clippy for both crates with `-D warnings`, formatting, and source-size checks.
- [x] Keep the storage ownership scanner unchanged and without exclusions. It reports 679 findings: 1 manifest dependency, 9 debug-only, 49 inline-test, and 620 test-file findings.
- [x] Commit this cutover as `e514a629`.

## Initial-sync snapshot persistence tests

- [x] Move the two `collect_entities_page` persistence tests out of Core; seed objects through Store APIs and keep merge-journal schema assertions under ArkStore.
- [x] Leave actor, authenticated-apply, and replay behavior assertions in Core.
- [x] Verify ArkStore (153/153), ArkCore (393/393), Clippy for both with `-D warnings`, formatting, and source-size checks.
- [x] Keep the storage ownership scanner unchanged and without exclusions. Current count is 673: 1 manifest dependency, 9 debug-only, 49 inline-test, and 614 test-file findings.
- [x] Commit as `a3e98a0b`.

## Production actor recovery fixture ownership

- [x] Move interrupted-restore database setup, marker SQL, restore intent manipulation, and capsule-path handling into Store test support.
- [x] Keep actor startup readiness, recovery refusal, rollback preservation, and recovery artifact cleanup assertions in Core.
- [x] Verify 21 production actor tests and 154 Store tests, Clippy for both crates with `-D warnings`, formatting, source-size, and pre-commit checks.
- [x] The unchanged storage-boundary scanner reports 663 findings, down from 673.
- [x] Commit as `fc27339a`.

## Actor write-failure test ownership

- [x] Move probe-table setup, failed mutation SQL, deferred commit violation, page-limit PRAGMAs, and persisted-state reads into Store test support.
- [x] Keep `SQLITE_BUSY`, `SQLITE_FULL`, commit failure, retry, rollback, and fail-closed behavior assertions in Core.
- [x] Verify all 21 production actor tests, all 154 Store tests, Clippy for both crates with `-D warnings`, formatting, diff check, and source-size gate.
- [x] The unchanged storage-boundary scanner reports 636 findings, down from 663.
- [x] Commit as `89fface7`.

## Actor restore test fixture ownership

- [x] Move restore-intent seeding, SQL marker reads, intent corruption, and recovery-capsule removal into the feature-gated `ark-store::test_support::InterruptedRestoreFixture`.
- [x] Keep startup readiness, `RecoveryRequired`, rollback preservation, and recovery artifact cleanup assertions in Core.
- [x] Verify all 21 production actor tests, all 154 ArkStore tests, Core and Store Clippy with `-D warnings`, formatting, source-size, and diff checks.
- [x] Keep the storage ownership scanner unchanged and without exclusions. It reports 663 findings, down from 673.

## Production actor log test ownership

- [x] Move actor-test log schema, writes, and reads from Core actor behavior tests into typed ArkStore test support.
- [x] Preserve insertion-order and sequence-order assertions, panic rollback checks, admission counts, and shutdown persistence behavior in Core.
- [x] Verify production actor tests (21/21), ArkStore tests with `test-support` (154/154), and Clippy for Core and Store with `-D warnings`.
- [x] Verify formatting, diff whitespace, and source-size gate. The unchanged storage-boundary scanner now reports 596 findings, including one production dependency finding; the remaining findings are test/debug references.
- [x] Commit this unit after the repository pre-commit checks pass (`8bfc2fb9`).

## Device registry test persistence reads

- [x] Replace the remaining direct SQL in Core device-registry behavior tests with Store registry, object, and sync-value reads.
- [x] Keep trigger-based storage fault injection in feature-gated ArkStore test support; keep Core assertions for actor outcomes, admission rejection, replay, restart, and transaction atomicity.
- [x] Verify all filtered Core device-registry tests (12/12), ArkStore tests with `test-support` (154/154), and Clippy for Core and Store with `-D warnings`.
- [x] Verify formatting, diff whitespace, and source-size gate. The unchanged storage-boundary scanner reports 559 findings, down from 583; the Core device-registry test family has no direct SQL, SQLite operations, or rusqlite references.
- [x] Commit this unit after the repository pre-commit checks pass (`8edecf19`).

## Paired-device enrollment test ownership

- [x] Keep Core assertions for enrollment outcome, retry idempotency, and reopen behavior while replacing direct persistence SQL with Store reads.
- [x] Move projection-failure trigger setup into feature-gated ArkStore test support; keep Core assertions for the mapped failure and atomic rollback of event, projection, and transport-key pin.
- [x] Verify the device-registry Core tests (12/12), Store tests with `test-support` (154/154), and Clippy for Core and Store with `-D warnings`.
- [x] Verify formatting, diff whitespace, and source-size gate. The unchanged boundary scanner reports 583 findings, including one production dependency finding.
- [x] Commit this unit after the repository pre-commit checks pass (`4abff1da`).

## Operation-receipt service test ownership

- [x] Replace Core SQL assertions about receipt rows and revision rows with public service behavior checks for replay, stable object revision, and replay after reopen.
- [x] Verify `ClearAll` permits reuse of an operation ID with a new payload and that the new object is observable through the service API.
- [x] Verify the focused service tests (3/3), full Core library tests (393/393), Core Clippy with `-D warnings`, formatting, diff check, and source-size gate.
- [x] The unchanged storage-boundary scanner reports 543 findings, down from 559. This unit removed 16 findings.
- [x] Commit this unit after repository pre-commit checks pass (`eca1afe9`).

## Legacy-write failure fixture ownership

- [x] Move legacy batch and project-delete trigger setup plus persisted rollback probes behind a Store-owned fixture opened by database path.
- [x] Keep Core's request failures and typed rollback invariants; make the fault scenarios independent so each trigger remains active without ordered teardown.
- [x] Verify the focused Core legacy-write test (1/1), all Store tests with `test-support` (154/154), Clippy for Core and Store with `-D warnings`, formatting, diff check, and source-size gate.
- [x] The unchanged storage-boundary scanner reports 537 findings, down from 543. This unit removes six Core test SQL findings and no longer passes a SQLite connection across this test-support API.
- [x] Commit this unit after repository pre-commit checks pass (`6784be74`).

## Legacy-write persistence snapshot ownership

- [x] Move all remaining direct SQL reads from the Core legacy-write test into a Store-owned typed snapshot.
- [x] Preserve assertions for version-vector HLCs, link revisions, tombstones, soft and hard deletion, applied receipts, committed-feed rows, task-merge bootstraps, and existence operations.
- [x] Verify the focused Core test (1/1), Store tests with `test-support` (154/154), Clippy for Core and Store with `-D warnings`, formatting, diff check, and source-size gate.
- [x] The unchanged storage-boundary scanner reports 524 findings, down from 537. The Core `legacy_writes.rs` test file now has zero direct SQLite or SQL findings.
- [x] Commit this unit after repository pre-commit checks pass (`83564acd`).

## Type-mutation service test ownership

- [x] Replace Core's direct SQL reads with the public type-read API and move pending-object setup, revision-failure triggers, and rollback inspection into a Store-owned fixture.
- [x] Preserve assertions that failed registration leaves the type absent, keeps pending data, emits no replay notice, and that retry registers the type, consumes pending data, writes the object, and emits one notice.
- [x] Verify focused Core type-mutation tests (2/2), Store tests with `test-support` (154/154), Clippy for Core and Store with `-D warnings`, formatting, diff check, and source-size gate.
- [x] The unchanged storage-boundary scanner reports 513 findings, down from 524. The Core `type_mutation_commands.rs` test file now has zero direct SQLite or SQL findings.
- [x] Commit this unit after repository pre-commit checks pass (`6adffa49`).

## Operation-receipt test ownership follow-up

- [x] Restore behavioral evidence lost when Core's generic counter fixture was removed: count actual object row insert/update effects during concurrent duplicate requests and payload conflicts.
- [x] Exercise receipt insertion failure through Store's real object-delete transaction and verify both object state and receipt state roll back.
- [x] Verify Store (170/170), Core library tests (378/378), targeted receipt tests, Clippy on all targets with `-D warnings`, formatting, source-size, and diff checks.
- [x] Independent no-comments review found no actionable issues. Commit `a8e8bb15` contains this follow-up.
- [x] Re-scan the unchanged boundary gate: 422 findings remain (1 production manifest dependency, 363 test-file, 49 inline-test, 9 debug-only).
