# Canonical game upsert cutover

- [x] Ground current actor, application adapter, SQL projection, and accepted closed Store-operation boundary.
- [x] Compare independent cutover candidates; both support Store-owned transaction and Application-owned DTO/decision with no Core dependency.
- [x] Move game wire/domain values and pure validation/planning to `ark-application` while preserving Core re-export paths.
- [x] Move game reads and persistence into `ark-store`; replace nested savepoint with one Store-owned immediate transaction.
- [x] Route game upsert through the existing Core writer actor admission/settlement path; remove the remaining `ObjectMutationSession` adapter.
- [x] Verify no-op, upsert, links/local/quarantine, rollback, RPC wire/event behavior, and affected crate gates.
- [x] Deslop and thermonuclear-review the diff; replace the positional game-property tuple with a named type. Commit the slice after targeted verification.

## Throughput checkpoint

Application planner tests (4), `phase3_game_rpc` tests (11), and the full Store suite (90) pass. The affected-crate check, source-size gate, formatting, diff check, strict Application/Store Clippy, and Core library Clippy pass. Combined all-target Core Clippy is blocked by the unrelated existing `unreachable!` lint in `relay_sync/registry_fanout/sqlite_policy_tests.rs:311`. Review artifacts are in `/tmp/cortex-thermo-ark-exocortex/game-cutover-review/`.
