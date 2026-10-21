# KOS-428 panic admission boundary

## Architect

- [x] Ground
- [x] Sketch
- [x] Agree
- [x] Implement
- [ ] Scrap

## Arena

- [x] Frame
- [x] Fan out
- [x] Cross-judge
- [x] Pick
- [x] Graft
- [x] Verify

## Work

- [x] Confirm the production actor panic path and the KOS-428 acceptance mismatch.
- [x] Compare terminal actor failure with verified in-process reopen.
- [x] Select the smallest lifecycle contract that meets KOS-428 and preserves outcome-unknown semantics.
- [x] Implement only the accepted lifecycle change and update its behavioral test.
- [x] Run focused tests, full ark-core suite, format, lint, and diff checks.
- [x] Record the decision and verification in the KOS-428 recovery audit.
- [ ] Commit only the named source, test, and audit files.

## Grounding and synthesis

`KOS-428` says no service may continue as Ready after a panic. Two existing handlers violate this rule. The database writer actor reopens SQLite and returns to Ready after a command panic. The `ArkService` request worker reopens the database and continues consuming requests after a handler panic.

The selected design makes a panic terminal for each owner. The database actor keeps its existing `Failed` state, cancels queued commands, drops its connection on exit, and leaves the active command outcome unknown. The request worker returns an unavailable error that states the request outcome is unknown, drops its receiver, stops sync, and shuts down the database owner. A new owner may return to Ready only through normal explicit startup and restore recovery. Receipts remain the only safe retry path for a command with a stable operation ID and matching payload.

Two GPT-6-Luna candidates and an independent cross-judge converged on fail-closed behavior. Candidate A covers both panic boundaries; Candidate B contributes the smaller one-way actor failure path. The synthesized change skips a new actor disposition type and an optional connection because the existing Failed lifecycle prevents later database work and the owner loop drops the connection on exit. It also avoids a new public error variant; the existing unavailable error carries explicit outcome-unknown wording.

Design artifacts: `/tmp/kos428-panic-architecture/candidate-a.md`, `/tmp/kos428-panic-architecture/candidate-b.md`. Cross-judge selected A with B's simpler actor lifecycle and queue cancellation. Accepted choice follows the literal KOS-428 acceptance and avoids adding recovery machinery behind cloned handles.

Verification: both focused panic tests pass. The full serialized `cargo test -p ark-core --lib -- --test-threads=1` suite passes 540 tests. `cargo clippy -p ark-core --lib -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `node scripts/check-source-size.mjs` pass. The source-size script reports 182 existing files over its 300-line target without failing.
