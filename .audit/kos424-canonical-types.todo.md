# KOS-424 canonical type ownership

- [x] Ground the registration, validation, and persistence dependency edges.
- [x] Select a bounded pure-domain slice and preserve core import paths.
- [x] Move canonical JSON definitions, registration builder, and validator together into `ark-domain`.
- [x] Keep registry ingress, migrations, normalization, and SQLite behavior in `ark-core`.
- [x] Verify domain/core behavior, dependency direction, formatting, Clippy, and source-size.
- [x] Commit this verified slice independently.

## Data shape

Frozen JSON definitions build ordered `TypeRegistration` values. The validator
consumes one registration plus `props` and `content` JSON values and returns a
typed `CanonicalValidationError`. Both are pure domain behavior. Registry
resolution, DB migration, and legacy normalization remain core adapters.

## Decision

Move definitions and validation together because content-contract validation
cross-checks the canonical registration list. Keep the same public paths under
`ark_core::canonical_types` as re-exports while callers remain in core.

The facade keeps `core_task_tests` in ark-core to pin those compatibility
paths. SQLite-backed migrations, ingress and compatibility planning remain
there. `ark-domain`'s dependency tree contains semver and pure data libraries,
with no SQLite, Tokio, iroh, or ark-core dependency.

Verification: `cargo nextest run -p ark-domain -p ark-core --test-threads 1
--no-fail-fast` passed 729 tests with one skipped; strict Clippy, all-target
check, formatting, source-size, and diff checks passed.
