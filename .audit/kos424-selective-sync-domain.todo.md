# KOS-424 selective sync domain rules

- [x] Move filter parsing, canonicalization, and evaluation into `ark-domain`.
- [x] Move selective entity projection into `ark-domain`; retain the existing
  `ark_core::data_platform` paths as re-exports.
- [x] Keep profile persistence and accepted-peer SQL in `ark-core`.
- [x] Run full serial ark-domain/ark-core tests, strict Clippy, dependency-tree,
  formatting, diff, and source-size checks.
- [x] Commit this verified boundary independently; keep KOS-424 open.

The moved logic consumes `SyncEntity` and policy values, then projects outbound
payloads. It does not mutate stored entities. SQL profile reads/writes and
policy acceptance remain in core. Existing two-database policy tests still prove
that narrowing stops transfer while retaining the sender's local data, and
widening can backfill.
