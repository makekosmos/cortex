# Task: ARK object migration regression tests

## Context

Delphi tasks and Eden notes now migrate automatically into ARK objects. This task adds regression coverage for the migration behavior and removes stale Delphi guidance that still points at the removed `delphi-db` sidecar.

## Acceptance Criteria

AC1. Delphi object-first migration is covered by tests for legacy todo migration into `task_obj`.

AC2. Delphi migration tests cover idempotency and object-first reads/writes without requiring the removed `delphi-db` sidecar.

AC3. Eden object migration is covered by tests for Heart note types/entries migrating into ARK object types/objects.

AC4. Eden migration tests cover link migration after target objects exist.

AC5. Delphi local documentation no longer presents `delphi-db` as an active runtime path.

AC6. Fresh verification runs relevant Delphi/Eden checks and records evidence.
