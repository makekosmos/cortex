# KOS-424 shared task-merge delivery

## Contract

Move policy-bound outbound task-merge preparation and acknowledgement into `ark-sync`. Keep authentication, peer readiness, capability checks, fanout, and network sends in `ark-core` transport adapters. A device policy that excludes task data suppresses transfer and never deletes local data.

## Architect phases

- [x] Ground
- [x] Sketch
- [x] Agree
- [x] Implement
- [ ] Scrap

## Implementation

- [x] Move the accepted-policy identity read into a shared policy port and type policy refusal.
- [x] Add one shared outbound coordinator in `ark-sync`; migrate LAN and relay callers in the same unit.
- [x] Add coordinator behavior tests; run real LAN transport test and relay admission/rejection tests.
- [x] Scoped checks: `cargo check --locked -p ark-core --tests`, strict Clippy for `ark-sync` + `ark-core`, rustfmt, source-size, and format checks pass; 55 `ark-sync` tests and 31 focused `ark-core` tests pass, including real LAN delivery over the wire.
- [x] Record limitations: full `ark-core` nextest passes on serialized rerun (607 passed, 1 skipped); an earlier parallel run had one transient Relay SQLite catch-up failure. `pnpm run check:affected` is blocked by 17 mismatches in the SQLite writer census allowlist across existing paths; the direct writer check reproduces it.
- [ ] Review the final diff and commit only this unit.
