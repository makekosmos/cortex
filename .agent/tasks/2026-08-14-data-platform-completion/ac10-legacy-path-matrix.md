# AC10 Windows legacy-path matrix

Checked 2026-09-08 against merged Core `ba13c5b5aed364ef1b6704b5e08a98bc1898964a`,
Cortex `199a4e379c5423c2e9b87cc46d000a0fefd53704` and Engine release `v0.1.2`.

## Literal protocol-retention condition

Historical post-window audit at Core commit `07eb1eb5` records the exact
condition: `tracking_started_at=2026-07-31T16:02:09.107277400Z`,
`legacy.connections=0`, `legacy.last_seen=null`, zero `legacy:` client
buckets, and `legacy_zero_for_30_days=true` after 31 complete days at
`2026-08-31T21:29:24.7197843Z`; earliest eligibility was
`2026-08-30T16:02:09.107277400Z`. The 30-day condition is therefore **PASS**
for the protocol-removal review gate. It is not authorization to delete the
compatibility WS path, which still requires the separately reviewed cleanup
slice and migrated-consumer proof.

| Frozen path/behavior | Current tracked location and owner | Usage/replacement evidence | Result |
| --- | --- | --- | --- |
| Manifest v1 broad grants | `cortex/desktop/scripts/package-release.mjs`, `runtime/src/runtime_grants.rs`; Desktop/Cortex | Package-release rejects legacy v1; v2 grants compile from Core canonical registry | PASS/guarded |
| App-owned canonical registration | `core/crates/ark-core/src/canonical_types/definitions`, `cortex/runtime/src/package_registration.rs`; Core/Cortex | Type-registry `13/13`; package registration reads Core canonical registrations and adds only package-owned definitions | PASS/guarded |
| Legacy planning writes/tables | Core `crates/ark-core/src/canonical_types/migration`, `db/legacy.rs`; Core | Source archive, identity/semantic parity, transactional retirement and reopen/idempotence tests | PASS/retired; `areas`/`headings` removed after lossless migration |
| HTTP-to-legacy-WS proxy | `cortex/runtime/src/engine_api`; Cortex | HTTP/WS socket parity and owner isolation `1/1`; no legacy proxy path found | PASS/guarded |
| Hardcoded Host access policy | `cortex/runtime/src/runtime_grants.rs`, `package_service`; Cortex | v2 launch-scoped typed grants use Core canonical registry; authority/worker suites pass | PASS/guarded |
| Obsolete Engine discovery paths | `cortex/desktop/electron/engine-discovery.test.ts`, `arca-sdk/src/ensure-engine.ts`; Desktop/SDK | Strict `engine.lock.json` discovery and incomplete-lock rejection `2/2` | PASS/guarded |

Core planning retirement is guarded by the canonical source archive and
transactional drop tests in `phase3_migration_system` (lossless archive,
reopen/idempotence, mismatch rejection and partial-drop rollback). Runtime
recovery is now migration-owned through typed
`packages.restore_migration_snapshot`; the obsolete rollback RPC and its
consumer are removed. Prepared null-token recovery uses the explicit source
set, while stale/mismatched tokens return `NotFound` without a new restore.

The isolated Phase 9 proof at Core commit `de9642fd` passes backup/reopen,
integrity/FK, restore, digest stability and idempotent rerun. Combined with
the Core retirement and Cortex typed-recovery proofs in
`raw-ac10-proof-20260908.md`, AC10 is `PASS_WINDOWS_ONLY`. Desktop obsolete
resolver removal is delivered by Cortex PR43 and remains covered by the
strict discovery tests.
