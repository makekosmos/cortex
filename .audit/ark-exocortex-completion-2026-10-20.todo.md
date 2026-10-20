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
- [ ] Move each remaining SQLite operation family to Store-owned typed capabilities, migrating callers and deleting Core SQL paths in the same unit.
- [ ] Remove Core SQLite source and Cargo dependencies, including test fixtures, and make the boundary gate pass.
- [ ] Complete the KOS-428 recovery matrix on supported platforms and map each acceptance criterion to evidence.
- [ ] Run the full local checks and desktop, mobile, and Windows CI on the final SHA.
- [ ] Run the independent correctness and thermo-nuclear code-quality reviews; resolve confirmed findings.
- [ ] Update architecture and recovery documentation, reconcile KOS-424, KOS-425, KOS-428, and KOS-412 in Linear, and record final evidence.

## Baseline

On 2026-10-20, `scripts/check-ark-core-storage-boundary.mjs` reported 846 findings: 162 production findings, 672 test-file findings, 10 inline-test findings, and 2 debug-only findings. This is a source scan, so each finding must be confirmed against the operation map before it is treated as a distinct migration unit.

The plan's earlier estimate of 15–25 active hours is stale. The current baseline contains 846 source findings and only local recovery evidence; re-estimate after the production operation families and test fixture moves are mapped.

## Current verified commits

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

## Latest boundary scan

The source scanner reports 845 findings after inbound receipt writes moved: 154 production, 676 test-file, 10 inline-test, and 5 debug-only. Production findings decreased by one, while test-file findings increased by two. The overall boundary gate still fails.

## Recovery baseline recheck

The plan's `signed_hpke_replication_reaches_offline_provider_and_rejects_stale_inputs` test is not present in this checkout or its Git history. The failed historical run `39087077632` was on `codex/ark-stage1-hpke` at `f7fd1709` and its failing test was `stale_restore_preserves_pending_usage_and_origin_sequence_before_peer_resync`; that test passes locally on the current branch. This does not satisfy the final Windows CI requirement. The last successful full CI run on this branch was `39136020199` at `42f93c2c`, before the current commits.

## Integration replication boundary batch

- [x] Frame one shared ownership rubric and inspect the current integration command, Store, and signed-frame paths.
- [x] Compare two structurally distinct transaction-boundary designs and get an independent judgment.
- [x] Select a typed Store write capability executed only after the existing weighted Core actor task is admitted; do not queue closures.
- [x] Move signed-frame transition validation to Application. Keep the replay callback result typed as `SignedSyncError`.
- [ ] Move integration use-case decisions into Application and SQLite row codecs, vector updates, replay reservation, savepoint, and transactions into Store.
- [ ] Keep Core's typed actor requests, `require_engine`, authenticated-peer extraction, registry admission, wire mapping, and service orchestration.
- [ ] Remove migrated Core SQLite operation helpers and callers in the same cutover.
- [ ] Run the integrated integration-replication behavior suite, crate checks, lint and local gates once for this family, then commit.

Synthesis evidence: `/tmp/ark-integration-candidate-actor.md`, `/tmp/ark-integration-candidate-contract.md`, and `/tmp/ark-integration-design-judge.md`. The typed contract candidate was selected because it fits the existing Store `WriteCapabilities` model and avoids callbacks in queued actor messages. The independent judge found both candidates omitted a typed scope for applying the entity batch under replay savepoint; this batch must preserve it as one scoped Store operation with typed failure. The use-case module currently lives at `ark-core/src/application/signed_integration.rs`, so moving it into `ark-application` is part of the work, not a presumed existing boundary.

The signed-frame Application policy move covers identity verification, pre-existing trust, node and grant epoch transitions, lease and credential fences, HLC checks, entity ordering, duplicate keys, and version-vector decisions. The SQLite session still lives in Core and currently supplies those reads and writes. `cargo test -p ark-application -p ark-core --lib` passed 19 Application tests and 431 Core tests after the move; formatting and diff checks passed. The source-size gate passed with the new policy validators split into `signed_integration/persistence.rs`.
