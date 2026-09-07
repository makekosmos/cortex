# Data Platform completion evidence bundle

Status: `BLOCKED` — evidence-only; no cleanup or legacy removal is authorized.

Captured 2026-09-07 against Core `origin/main`
`251995135f1b16f34fcc26f18e15978635930b9b`.

## Current delivered references

- Core: `251995135f1b16f34fcc26f18e15978635930b9b`.
- Cortex runtime release merge: `d85cf0c8e759b43b778fc2ce3fe43fdf02030eee`.
- Cortex current main: `f80650ab88cf2e9d99e9ba45f33c3dacc953b645`.
- Arca SDK reconnect path: `18008cc` (`quality/portable-hooks`).
- Windows Core RPC fixture: SHA-256
  `0224e0bda0bb03ad0bf66848321432a404636e8584b71c96f6c42d29e548e479`.

## Phase 1–2 reconciliation map

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

## Checks actually run

| Check | Result |
| --- | --- |
| Core focused canonical/migration/compatibility/Phase 7/sync suites | PASS: 140/140, 18 suites |
| `cargo test --workspace --all-targets --no-fail-fast` | PASS: 385 tests, 30 suites |
| `cargo fmt --check` | PASS |
| `node scripts/check-source-size.mjs` | PASS; 9 grandfathered files |
| `node scripts/check-ark-write-boundaries.mjs` | PASS |
| `node scripts/check-ark-generated.mjs` | PASS; existing ts-rs warnings only |
| `cargo clippy --workspace --all-targets` | PASS with 31 existing warnings |
| `cargo clippy --workspace --all-targets -- -D warnings` | FAIL: 22 existing baseline lint errors |
| Cortex `package_worker_windows` with `ARK_CORE_RPC_PATH` | PASS: 11/11 |
| Cortex `package_worker_process_windows` with `ARK_CORE_RPC_PATH` | PASS: 20/20 |
| Cortex `cortex_2_acceptance` with `ARK_CORE_RPC_PATH` | PASS: 7/7 |
| Cortex `desktop_authority_socket` with `ARK_CORE_RPC_PATH` | PASS: 6/6 |
| Arca SDK `bun test tests/engine-v1-contract.test.ts` | PASS: 3/3; strict discovery, coalesced reconnect and bounded idempotent replay |

## Frozen acceptance status

- AC1: `PARTIAL` — the reconciliation map above pins the existing accepted
  phase slices and their ownership. The Arca SDK reconnect production path is
  now `PASS` on its 3-test contract target; full cross-consumer Engine/Host
  acceptance is still `NOT_RUN` for remaining adapter and Windows/package
  lifecycle gates. Current Core ownership does not imply those implementation
  files must be in Core.
- AC2–AC9: partial evidence exists in the accepted phase/runtime references,
  but this bundle does not claim the full umbrella criteria without their exact
  phase-specific artifacts and command matrix.
- AC10: `NOT_RUN` as a full frozen matrix. The Core-supported rows above pass;
  full app/Host/desktop rows and their exact-head evidence are not present in
  this Core checkout. Windows/Darwin-only rows remain platform-specific.

## Phase 9 gate semantics

The frozen Phase 9 spec is
`origin/codex/engine-v1-data-platform-phase9-spec:.agent/tasks/2026-08-12-phase9-legacy-cleanup/spec.md`.
Its 30-day stable-release requirement applies to Gate 5 protocol-removal
telemetry (`legacy_zero_for_30_days`) only. It is not a blanket wait before
cleanup. The separate cleanup proof bundle is still incomplete: no accepted
immutable Phase 3–8 implementation manifest, final backup/reopen/restore and
count/ID/digest bundle, or exact idempotent rerun proof has been delivered on
this Core main.

No destructive cleanup is permitted from this bundle. The next actions are to
attach the remaining exact phase artifacts and cross-consumer Engine/Host
evidence, then re-run the frozen supported matrix against one final candidate.
The reconnect client contract itself is no longer an untested residual.
