# AC10 Windows legacy-path matrix

Checked 2026-09-07 against Cortex `20fd15a90c3d429dd0f9fef025583f2992ed4862`
and its clean tested head `f4d972cd176f03338fe98ea77c4603db63e1594e`.

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
| Legacy planning writes/tables | Core `crates/ark-core/src/schema.rs`, `db/legacy.rs`, `canonical_types/migration_registry`; Core | Read-only guard `1/1`; Area/Heading writes fail closed, new writes use canonical objects, migration readers remain live | RETAINED read-only migration boundary; contract-compatible under AC1/AC2 |
| HTTP-to-legacy-WS proxy | `cortex/runtime/src/engine_api`; Cortex | HTTP/WS socket parity and owner isolation `1/1`; no legacy proxy path found | PASS/guarded |
| Hardcoded Host access policy | `cortex/runtime/src/runtime_grants.rs`, `package_service`; Cortex | v2 launch-scoped typed grants use Core canonical registry; authority/worker suites pass | PASS/guarded |
| Obsolete Engine discovery paths | `cortex/desktop/electron/engine-discovery.test.ts`, `arca-sdk/src/ensure-engine.ts`; Desktop/SDK | Strict `engine.lock.json` discovery and incomplete-lock rejection `2/2` | PASS/guarded |

Core planning boundary check: `cargo test --manifest-path
crates/ark-core/Cargo.toml legacy_planning_writes_are_read_only_without_sync_side_effects
--quiet` — `PASS: 1 passed, 384 filtered out`. Area/Heading writes return
`LegacyPlanningReadOnly` without changing any table counts, sync vector or
tombstones. The remaining Todo/Project/Tag compatibility facade writes
canonical objects and sync entities, so `db/legacy.rs` is not an obsolete
planning-writer deletion target without a replacement API and migration proof.

Runtime package rollback is likewise migration-owned: `desktop/electron/
legacy-migration-runtime.ts` calls `packages.rollback_legacy_grants`, which is
implemented by `runtime/src/ws_server/ops/package_legacy.rs`. Removing that
operation now would break prepared-journal recovery; retain it until the
replacement migration protocol is implemented and tested.

The isolated Phase 9 proof at Core commit `de9642fd` passes backup/reopen,
integrity/FK, restore, digest stability and idempotent rerun, but it is not a
proof that the listed legacy paths have been removed. AC10 therefore remains
`NOT_RUN`/open: all six frozen Windows behaviors and the 30-day condition pass,
but Docs review confirms AC10's explicit removal requirement still applies to
the retained planning tables/readers and migration rollback handlers. AC1/AC2
compatibility explains their current use but does not satisfy removal. A
replacement migration/recovery contract, consumer/source guards and reviewed
deletion proof are required. Desktop obsolete
resolver removal is delivered by Cortex PR43
(`a43dbe90`, merged as `c630b4c77a41e7d47f8db514995455143b93ee16`).
