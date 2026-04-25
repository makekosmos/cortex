# Verification Problems

## P1: Verifier was too whitespace-sensitive

The first `verify-ffi-relay-wiring.ts` run failed because it looked for the exact single-line snippet `relay.broadcast_live_change(entity.clone())`. `cargo fmt` split that call across multiple lines, while the actual relay broadcast wiring was present and `cargo check` passed.

### Fix

Normalize whitespace in the verifier before checking snippets. No production code change was needed for this problem.
