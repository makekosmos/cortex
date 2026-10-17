# Cortex ARK epic continuation

## Phase A: Frame

- [x] Read the Principles section of the `poteto-mode` skill.
- [x] Ground first, then commit. Don't start the run until you can state:
  - [x] The definition of done as a falsifiable predicate.
  - [x] Scope, quantified: three remaining Linear workstreams, each split into independently verified commits; the full code and CI matrices still need current-state verification.
  - [x] The rigor level, biased high. One-way doors and high blast radius get more. Reversible low-stakes steps get less. Rigor is gates and artifacts, not "try harder".
- [x] Present the framing and tradeoffs before committing to a long run.

Completion predicate: all 17 children of KOS-412 are Done; KOS-412's user-facing contracts and technical acceptance criteria are proven on current source and runtime; KOS-424 has the target Cargo ownership graph with migrated callers and obsolete internal paths removed; KOS-425's forbidden-edge/feature tests and supported desktop/mobile CI release matrix pass on GitHub; KOS-428 proves safe backup/restore, durability boundaries, no identity or origin-sequence reuse, and fail-closed recovery behavior; required repository gates and independent final review pass. Do not close the epic on a passing subset.

Current Linear snapshot: KOS-424, KOS-425, and KOS-428 are In Progress. KOS-425 depends on KOS-424. Recheck all 17 children directly before final status updates. The local branch has no PR and no remote workflow run; CI-only acceptance still needs an authorized published ref. Do not push without explicit authorization.

Rigor: high. The work changes replicated personal data, device admission, SQLite recovery, and crate/build boundaries. Existing dirty `.audit` files are outside this run's ownership; preserve them and stage only named run artifacts and implementation files.

## Phase B: Design the workflow

- [x] Decompose into atomic, independently-landable units. Sequence riskiest-unknown-first. Scaffold and verification come before features.
- [x] Build the verification harness before the work, with the baseline captured from the pre-change state, so the check reads as "old value vs new value".
- [ ] For one-way-door design decisions, run the architect skill. Skip it for mechanical work whose shape is already concrete.
- [x] Decide what fans out. Parallelize only across seams, and give each worker its own worktree or branch. Don't over-fan.
- [x] Write the designed phase list down. That list is what the human reviews.

Designed phases:

1. Reconcile all 17 Linear children and the live source tree; map each remaining acceptance item to files, tests, CI evidence, and current status.
2. Close KOS-428 first because stale restore and commit durability can affect user data. Reproduce each missing failure case on production SQLite; fix only demonstrated gaps; run macOS and available Windows evidence; keep unsupported power-loss claims explicit.
3. Finish KOS-424 bottom-up. Ground the remaining relay/sync orchestration boundary, sketch alternatives only if ownership is still open, move one contract at a time, migrate callers, then delete obsolete internal exports.
4. Finish KOS-425 after KOS-424. Prove the whitelist rejects an injected forbidden edge and mobile-only feature leak; run desktop/mobile release jobs and capture GitHub results.
5. Audit all 17 child descriptions and cross-cutting user requirements; run final root checks, independent diff review, and reconcile issue/epic status only when evidence meets acceptance.

## Phase C: Run the loop

- [ ] Each unit is an experiment. State the hypothesis, make the smallest change, measure against the predicate on the real artifact, keep it if it advanced, revert it if it didn't.
- [ ] Verify by inspecting the artifact, never a self-report. When something passes too easily, suspect the observation method before the system.
- [ ] Pair delegated work with a judge. If a worker games the gate, reset and harden the contract. If the gate itself is wrong, fix the gate in its own change rather than routing around it.
- [ ] A verdict is VERIFIED, NOT VERIFIED, or INCONCLUSIVE. Inconclusive is not a pass. Don't hide a negative.
- [ ] Throughput checkpoint: after each commit, refresh the next issue's source map and acceptance evidence before editing it.

## Phase D: Keep the audit trail

- [ ] Log the run via the `show-me-your-work` TSV trail at `.audit/ark-exocortex-run-2026-10-17.tsv`.
- [ ] Audit the log against this run's transcript and verify every evidence pointer before handback.
- [ ] Cross-model review the trail before handback.

## Phase E: Verify and hand back

- [ ] Check the whole against the Phase A predicate on the real product, not just the harness.
- [ ] Encode any recurring correction as a gate, lint rule, test, or script.
- [ ] Verify current files, tests, platform builds, live Linear statuses, the final commit diff, and the decision trail.

## Work units

- [ ] Recheck all 17 Linear children and refresh the contract-to-evidence matrix.
- [ ] KOS-428: audit missing cases; implement and commit verified fixes; run supported-platform durability checks.
  - [x] Add a production actor test proving shutdown returns at deadline while started work finishes and new work is refused.
  - [x] Reconcile the retry-across-restore wording with the selected watermark refusal contract; run current restore-focused tests.
  - [x] Type restore failure separately from safe refusal and recovery-required; prove a real SQLite restore failure with successful rollback still fails the actor closed.
- [ ] KOS-424: finish the remaining sync orchestration extraction and remove obsolete internal paths.
  - [x] Migrate `ark-core` callers to direct `ark-application` / `ark-domain` dependencies and remove the obsolete `application::sync_storage` and `domain` facades.
- [ ] KOS-425: prove negative graph/feature fixtures and capture macOS, Windows, and Android release CI results.
- [ ] Final audit, independent review, issue status reconciliation, and epic closure when every criterion is proven.
