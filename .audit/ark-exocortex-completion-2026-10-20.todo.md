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

## Latest boundary scan

The source scanner reports 844 findings: 158 production, 672 test-file, 10 inline-test, and 4 debug-only. Remaining production findings keep the overall boundary gate failing.

## Recovery baseline recheck

The plan's `signed_hpke_replication_reaches_offline_provider_and_rejects_stale_inputs` test is not present in this checkout or its Git history. The failed historical run `39087077632` was on `codex/ark-stage1-hpke` at `f7fd1709` and its failing test was `stale_restore_preserves_pending_usage_and_origin_sequence_before_peer_resync`; that test passes locally on the current branch. This does not satisfy the final Windows CI requirement. The last successful full CI run on this branch was `39136020199` at `42f93c2c`, before the current commits.
