# Cortex ARK epic completion

## Phase A: frame

- [x] Read `poteto-mode` principles and current repository instructions.
- [x] Recheck the live Linear epic and all 17 child statuses.
- [x] Define completion from KOS-412's cross-cutting acceptance criteria.
- [x] Preserve existing dirty `.audit/` artifacts and stage only named deliverables.

Completion predicate: all 17 children of KOS-412 are Done; each child has code, documentation, or verification evidence matching its own acceptance criteria; the full supported host/build matrix and applicable tests pass; the final diff and decision log agree with repository state. A passing subset does not close the epic.

Scope at pickup: 9 children Done, KOS-429 In Progress, 7 children Backlog. KOS-430 is a separate Done fix under KOS-418. The checkout already contains foundational changes through commit `0950b184`; 69 untracked audit artifacts predate this continuation and must not be staged wholesale.

Rigor: high. The work changes persistent storage, authorization, replication and build boundaries. Each unit must prove behavior on production SQLite or the actual target build before the next dependent unit starts.

## Phase B: design the workflow

- [x] Revalidate remaining issue statuses and the full completion predicate from Linear.
- [ ] Build a source map and before-state check for each remaining issue before its implementation.
- [x] Ground KOS-429 session lifecycle, transport authentication and every data-plane entry point.
- [x] Produce two independent KOS-429 architecture sketches with explicit types, signatures and module ownership.
- [x] Cross-judge the sketches, screen design red flags and reconcile the chosen contract with the source map.
- [ ] Implement KOS-429 in separately verified commits, updating the contract for any accepted deviation.
  - [x] Gate new pairing on endpoint-bound `PairingAccepted`; verify pending data isolation.
  - [x] Add the signed registry event domain model, canonical encoding, and verification tests.
  - [ ] Add signed registry events, durable projection, registry exchange, and shared `ReadyPeer` admission.
- [ ] Implement and verify KOS-429, then KOS-422.
- [ ] Implement and verify KOS-423, KOS-426, KOS-427 and KOS-428 against their separate acceptance criteria.
- [ ] Extract stable domain/application/store/sync/net modules into crates for KOS-424.
- [ ] Add and run the dependency and desktop/mobile build matrix for KOS-425.
- [ ] Reconcile issue statuses, epic acceptance criteria, source gates, tests, docs and final review.

Execution order: KOS-429 -> KOS-422 -> KOS-423 -> KOS-426 -> KOS-427 -> KOS-428 -> KOS-424 -> KOS-425. This keeps policy and merge behavior inside the current crate until their contracts settle, then extracts the finished boundaries.

## Phase C: run the loop

- [ ] For each unit, record its hypothesis and a before-state check.
- [ ] Make one bounded change, run the issue-specific production test or build, inspect the artifact, and commit the verified result.
- [ ] Reconcile accepted deviations with the design artifact before starting dependent work.
- [ ] Reopen a design phase when repeated workarounds show the chosen boundary is wrong.
- [ ] Throughput checkpoint: after every verified commit, refresh the next issue's source map and acceptance evidence before editing it.

## Phase D: keep the audit trail

- [ ] Append material decisions, verification results, pivots and blockers to `.audit/ark-exocortex.tsv`.
- [ ] Keep task prompts and scratch candidate output outside the shared source tree.
- [ ] Review the trail against this run's actual actions before handback.

## Phase E: verify and hand back

- [ ] Verify every child issue on the real source, database adapter, feature graph or supported target it names.
- [ ] Run the full required gates after the final change.
- [ ] Run an independent review of the complete final diff and the audit trail.
- [ ] Mark KOS-412 complete only after all 17 child issues and the cross-cutting predicate are proven.
