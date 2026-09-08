# Data Platform completion evidence bundle

Status: `PASS_WINDOWS_ONLY` — the agreed Windows/headless delivery scope is
complete. Non-Windows, visible-GUI, autostart, physical-microphone and Groq
checks are explicitly deferred by the user and are not claimed here.

Captured 2026-09-08 against merged Core PR #76 at
`ba13c5b5aed364ef1b6704b5e08a98bc1898964a`.

## Exact evidence links

- Frozen spec: https://github.com/makekosmos/core/blob/056c71d585a1af4e3cc7c47eb05dae98f61ef5c8/.agent/tasks/2026-08-14-data-platform-completion/spec.md
- Delivery PR: https://github.com/makekosmos/core/pull/76
- Cortex AC10 PR: https://github.com/makekosmos/cortex/pull/49
- Engine AC10 release: https://github.com/makekosmos/desktop/releases/tag/v0.1.2
- Compatible Desktop/Host release: https://github.com/makekosmos/desktop/releases/tag/v0.9.22
- Current evidence files: `evidence.md`, `evidence.json`, `problems.md`,
  `phase3-8-manifest.json`, `ac10-legacy-path-matrix.md`, and
  `raw-ac1-agenda-host-restart-20260907.md`, `raw-ac10-proof-20260908.md` in
  this PR. The independent Docs
  verdict is recorded in `docs-independent-review-20260907.md`.
- Historical Phase 9 proof source: https://github.com/makekosmos/core/commit/de9642fda19aa9d4284fb873b05e8024feab3203
- Final merged Core commit: `ba13c5b5aed364ef1b6704b5e08a98bc1898964a`.

## Current delivered references

- Core: `ba13c5b5aed364ef1b6704b5e08a98bc1898964a` (merged PR #76); AC9
  transport/tombstone closure remains pinned at `dfae4a86`.
- AC8 fixture commit: `f1e53e486c4b7e18941058f094f28d5cf775e505` (`db/tests_sync/part4.rs`).
- AC9 profile-contract commits: `3546d301`, loopback test `320772d2`, owner/revision persistence `5e348a01`, typed tombstones `377dd748`, and transport matrix `8d5db812`; focused profile suite `6 passed`, tombstone suite `1 passed`, transport matrix `1 passed`, and full local suite `439 passed`.
- Evidence bundle commit: `a83faaf588b95f6cebd718a9526c843946ef0f88` (merged evidence PR #77); this follow-up pins the compatible Desktop/Host release.
- Delivered PR code head: `ba13c5b5aed364ef1b6704b5e08a98bc1898964a`.
- Cortex current main: `199a4e379c5423c2e9b87cc46d000a0fefd53704` (merged AC10 PR #49).
- Cortex permission contract: PR39 merge `56ba941162bb15ae80644911f323811b4e133c5a`;
  implementation `4540de750d849fe83a25454da003aaa5551d6489`.
- Published Engine `0.1.2` archive SHA-256:
  `0874713b6793656d653ce5bb60d71c4393eee788dca1f1f9e73d67f8fabffd63`.
- Store catalog17 asset SHA-256:
  `f49ee76f44d3f3aef12829ba8e1841652e6ce1f6f2347c467782f79498922550`;
  Package Index catalog18 asset SHA-256:
  `1e65f56a3d5f6cb382619d4f8211a4dc5a6fdac019eb1c5592b99072a68c633f`.
- Follow-up published Package Index `catalog-19` release:
  `https://github.com/makekosmos/package-index/releases/tag/catalog-19`,
  catalog SHA-256
  `fbfc5853e75ec0f2f45cdc4ef7790ebdc217cf658fb91c166bec5fd8b0baa265`;
  Memoria `v0.6.7` release:
  `https://github.com/makekosmos/memoria/releases/tag/v0.6.7`, archive SHA-256
  `d1b5f39c6f06b2b6a760e27ed7ece49f3d8d51079d8901f8f1c0c080737c9023`, size
  `2886184`.
- Packaged-host acceptance source: `raw-ac10-proof-20260908.md` and
  `raw-windows-headless-20260908.md`.
- Core RPC fixture source: `0b06342014238244749946bbe3db92d94dfa0868` (Core repository).
- Arca SDK reconnect path: `18008cc` (`quality/portable-hooks`).
- Windows Core RPC fixture: SHA-256
  `0224e0bda0bb03ad0bf66848321432a404636e8584b71c96f6c42d29e548e479`.

## Phase 1–2 reconciliation map

The immutable Phase 3–8 historical pin manifest is
`phase3-8-manifest.json`. It records repository, frozen spec, implementation
and evidence SHAs; Phase 7/8 entries explicitly retain their pending limits.

The actual AC10 path/guard matrix is `ac10-legacy-path-matrix.md`. It records
the passing v1 rejection, discovery, planning-retirement and typed
migration-recovery guards. The raw closure commands and immutable artifact
hashes are in `raw-ac10-proof-20260908.md`.

AC2–AC9 are itemized in `ac2-9-gap-matrix.md`. AC3–AC9 now have explicit
Windows-only headless evidence; only non-Windows and visible GUI rows remain
deferred by user.

The existing phase proofs provide accepted Linux slices; they are not silently
promoted to a whole-program PASS:

| Frozen slice | Exact implementation | Current owner / result |
| --- | --- | --- |
| Direct Engine dispatcher | `316e36d61860458412db0e3ed048076d5c542520` | Cortex/Engine; `ACCEPTED_LINUX`, Windows/Electron `NOT_RUN_UBUNTU` |
| Reconnecting EngineClient | `79adbbb6cacbcae5e61b2a6be3ef7b090cff3529` | SDK/Host/Manager/Desktop; `ACCEPTED_LINUX`, Windows GUI/process `NOT_RUN_UBUNTU` |
| Supervisor control | `6c40b3e05859404dd1fafb7e1d0cce09f4682851` | Cortex/Engine; `ACCEPTED_LINUX`, Windows packaged lifecycle `NOT_RUN_UBUNTU` |
| Worker containment | `6d8011fcf4d7f5a2ca91beaf062cd04dec7cb382` | Cortex/Engine; Linux accepted, Windows execution was `NOT_RUN_UBUNTU` in the source proof |
| Package web isolation | `61b5b74e04f898bfd6935649cbf00ff78697e6e7` | Desktop/Host; `ACCEPTED_LINUX`, packaged Windows/Darwin `NOT_RUN_UBUNTU` |
| Versioned Type Registry | `25f7837c1e09871e4c50aeaba257b49802c430c9` | Core; `ACCEPTED_LINUX`, Windows/Darwin `NOT_RUN_UBUNTU` |

The current Cortex release line additionally has real Windows headless runs:
`package_worker_windows` 11/11, `package_worker_process_windows` 20/20,
`cortex_2_acceptance` 7/7, and `desktop_authority_socket` 6/6 with the pinned
Core RPC fixture. These cover process/job containment, worker crash and
cleanup, PID/credential/generation fencing, and disconnect cleanup. They do not
replace the full Engine v1 reconnect contract or any visual GUI gate.

The packaged candidate additionally passed the Manager/Host/Runtime install,
Store refresh, bridge and cleanup flow in `raw-windows-headless-20260908.md`.

The compatible Desktop/Host `v0.9.22` release is built from Cortex source
`f70f25e5bd29d274ad0455c862584002e2d5850b`, pins Core
`ba13c5b5aed364ef1b6704b5e08a98bc1898964a`, and uses Engine `v0.1.2`.
The Windows installer SHA-256 is
`44e51aa328717f0c4f1c1caac94883cb7b367b667a12616e063f5cef34981bd6`.
The published channel verifier passes version, hash, size, blockmap and
timestamp checks. Its packaged candidate smoke passes Manager/Host/Runtime,
Store/bridge, headless Dictation start/cancel and owned-process cleanup.
Five of six existing first-party release contracts pass; the remaining
Memoria crash-boundary fixture still misses its pre-crash marker and is
recorded as a pre-existing release-fixture residual outside the AC10 typed
recovery path.

Signed Memoria crash/restart acceptance on the Desktop `0.9.21` packaged
candidate: `PASS`, 1/1, 34.7s. The published follow-up against catalog19 and
Memoria `0.6.7` also passed `1/1` in `21.7s`, using the v0.9.22 source Host's
real contextBridge IPC with the packaged v0.1.2 Engine/ARK binaries. It
verified signed catalog apply/install/enable, the import crash boundary,
restart, rollback, and cleanup. This remains headless evidence and does not
close the visual GUI row.

Additional AC1 Windows cross-consumer proof on Cortex
tested HEAD `f4d972cd176f03338fe98ea77c4603db63e1594e`, based on canonical
`20fd15a90c3d429dd0f9fef025583f2992ed4862`:
`bun run --cwd host e2e first-party-agenda-contract.spec.ts --workers=1` —
signed Agenda install, typed ARK upsert, Engine restart, package listing and
Host restart: `PASS`, 1/1, 3.1m.
Sanitized raw evidence is committed at
`raw-ac1-agenda-host-restart-20260907.md` (SHA-256
`bdb32ef24e41c8cc62d93bdf9855ae8729c5f08e4b1e0a789977c760aa09daee`).

## Checks actually run

| Check | Result |
| --- | --- |
| Core focused canonical/migration/compatibility/Phase 7/sync suites | PASS: 140/140, 18 suites |
| `cargo test --workspace --all-targets --no-fail-fast` | PASS: 385 tests, 30 suites |
| `cargo fmt --check` | PASS |
| `node scripts/check-source-size.mjs` | PASS; 9 grandfathered files |
| `node scripts/check-ark-write-boundaries.mjs` | PASS |
| `node scripts/check-ark-generated.mjs` | PASS on Windows; generated bindings clean after the delivery hook; 30 files, existing ts-rs warnings only |
| `cargo clippy --workspace --all-targets` | PASS with 8 existing warnings under the repository hook command |
| `cargo clippy --workspace --all-targets -- -D warnings` | FAIL: 22 existing baseline lint errors |
| GitHub CI run `34155827949` | NOT_RUN: both jobs were not started because the account billing/spending limit failed |
| Cortex `package_worker_windows` with `ARK_CORE_RPC_PATH` | PASS: 12/12 |
| Cortex `package_worker_process_windows` with `ARK_CORE_RPC_PATH` | PASS: 20/20 |
| Cortex `cortex_2_acceptance` with `ARK_CORE_RPC_PATH` | PASS: 7/7 |
| Cortex `desktop_authority_socket` with `ARK_CORE_RPC_PATH` | PASS: 6/6 |
| Cortex package-worker protocol unit matrix | PASS: 8/8 |
| Cortex phase-5 runtime grants | PASS: 6/6 |
| Cortex Store runtime catalog | PASS: 12/12 |
| ARK Markdown bridge package/runtime suites | PASS: 11/11 + 1/1 |
| Packaged Manager/Host/Runtime headless smoke | PASS: catalog sequence18; clean process/package teardown |
| Core AC10 planning retirement | PASS: merged Core `ba13c5b5`; 37 migration/retirement tests + 198 lib tests |
| Cortex AC10 typed recovery | PASS: merged Cortex `199a4e37`; 15 Bun tests, 18 grant-authority tests, full pre-push gate |
| Engine 0.1.2 distribution/install | PASS: archive `0874713b...ffd63`; 8/8 distribution/install tests |
| Arca SDK `bun test tests/engine-v1-contract.test.ts` | PASS: 3/3; strict discovery, coalesced reconnect and bounded idempotent replay |
| Core `phase3_migration` integration test | PASS: 13/13 |
| Core `data_platform_phase7` | PASS: 6/6 on `dfae4a86`; profile modes/schema constraints, FilterV1 canonical digest, owner persistence and revision CAS |
| Core typed tombstone suite | PASS: 1/1; delete persistence, collection, type filtering and apply |
| Core selective-sync transport matrix | PASS: 1/1; full/metadata/none, ordering/references, exclusion, reconnect/replay, narrowing and delete |
| Core AC8 local-state regression | PASS: 1 passed, 199 filtered; device-keyed local state absent from export/sync/apply |
| Phase 9 immutable proof bundle rerun at `de9642fda19aa9d4284fb873b05e8024feab3203` | PASS: 1/1; backup/reopen/integrity/FK/restore, object count 1, digest stable, idempotent rerun |

## Frozen acceptance status

- AC1: `PASS_WINDOWS_ONLY`. Frozen requirement (spec frozen at `056c71d5`): “Accepted
  Phase 1 hardening and Phase 2 Versioned Type Registry changes are ported onto
  the current Lego baseline with no duplicate implementation. Their original
  focused tests, Engine v1 guards, and Package Host isolation/reconnect/lease
  behavior pass from the integrated tree. A commit map records source SHA,
  destination SHA, conflicts and explicit omissions.” Existing proof covers
  the pinned slices, Windows Host/process/authority targets, Arca SDK reconnect
  `3/3`, and signed Agenda Host↔Engine restart `1/1` with committed raw
  evidence. Visual GUI and
  non-Windows rows remain explicitly deferred/not run.
- AC2: `PASS_WINDOWS_ONLY` for the tested Core scope; cross-app fixture closure
  remains required for umbrella PASS.
- AC3–AC7: `PASS_WINDOWS_ONLY`; exact commands and immutable input pins are in
  `ac2-9-gap-matrix.md` and `raw-windows-headless-20260908.md`.
- AC8: `PASS_WINDOWS_ONLY`; the integrated per-object/per-device and
  no-export/no-sync regression passes on Windows (`1/1`, 199 filtered).
- AC9: `PASS_WINDOWS_ONLY`; profile projection is integrated into backend
  load/page and live broadcast paths. The actual loopback two-DB matrix covers
  note/task full, game metadata without local paths, definitions before
  objects, required object references, usage omission, reconnect/replay
  idempotence, non-destructive narrowing, and delete→typed tombstone→filter→
  apply. Owner-bound persistence and monotonic revision CAS pass in
  `data_platform_phase7` (`6/6`); full local nextest is `439/439`.
- AC10: `PASS_WINDOWS_ONLY`. Frozen requirement (spec frozen at
  `056c71d5`): “Only after AC1–AC9 pass and first-party consumers have migrated,
  remove Manifest v1 broad grants, app-owned canonical registration, legacy
  planning writes/tables (after preserved backup/migration evidence),
  HTTP-to-legacy-WS proxy, hardcoded Host access policy and obsolete Engine
  discovery paths. Canonical aliases remain. Source guards demonstrate the
  removed paths cannot return.” Existing packaged Windows, migration, local-AI
  and reconnect proof is recorded in the immutable Phase 3–8 manifest and
  historical Phase 9 proof bundle. The listed production guards are now
  PASS/guarded. Core planning retirement archives source rows, verifies
  identity/semantics and drops `areas`/`headings` transactionally; Cortex
  recovery uses the typed token/source-set contract and removes the legacy
  rollback RPC. The source guards, focused tests, packaged smoke and Engine
  0.1.2 manifest are recorded in `raw-ac10-proof-20260908.md`. Non-Windows
  and visible GUI rows remain deferred by user.

- AC16: `PASS_WINDOWS_ONLY` for the delivered Windows evidence. Non-Windows,
  visible-GUI, autostart, physical-microphone and Groq rows are explicitly
  deferred by the user. Frozen requirement (spec frozen at `056c71d5`): “Every
  implementation slice has frozen spec, evidence/evidence.json with exact
  commit and per-AC result, and independent verification against the final
  integration commit. Any failed or unavailable criterion has `problems.md`;
  the umbrella task is complete only after all delivery criteria pass or an
  explicitly user-approved follow-up replaces a non-PASS scope.” This update
  pins current Core/Cortex/Desktop references, Windows results, and the AC1/AC10
  artifacts. The independent Docs review and final merged Core commit are now
  pinned; hosted CI remains unavailable because the account billing/spending
  limit prevented job execution.

Cross-platform Debian/Linux/macOS, visible GUI, autostart, physical microphone
and Groq verification are `DEFERRED_BY_USER` / `NOT_RUN`; this Windows-only
delivery does not promote those rows to PASS.

## Phase 9 gate semantics

The frozen Phase 9 spec is
`origin/codex/engine-v1-data-platform-phase9-spec:.agent/tasks/2026-08-12-phase9-legacy-cleanup/spec.md`.
Its 30-day stable-release requirement applies to Gate 5 protocol-removal
telemetry (`legacy_zero_for_30_days`) only. It is not a blanket wait before
cleanup. The executable immutable Phase 3–8 manifest, backup/reopen/restore
counts, digest and idempotent rerun proof are now pinned in
`phase3-8-manifest.json` and `raw-phase9-proof-20260908.md`; cleanup
authorization remains separate.

The literal protocol-retention condition itself is already `PASS`: historical
post-window audit at Core `07eb1eb5` recorded
`tracking_started_at=2026-07-31T16:02:09.107277400Z`, zero legacy connections,
no legacy client buckets and `legacy_zero_for_30_days=true` after 31 complete
days at `2026-08-31T21:29:24.7197843Z`. This removes a timing blocker only; it
does not authorize deletion of the still-consumed WS compatibility path.

No destructive cleanup is permitted from this bundle. The explicit AC10
replacement migration/recovery contract and reviewed deletion proof are now
merged and verified for the Windows/headless scope. Core issue #34 can close
with the deferred rows recorded above. The local Whisper Turbo path was not a
blocker for this headless gate.
