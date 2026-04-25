# Task: ARK migration backup and telemetry

## Context

Automatic ARK object migrations exist for Delphi tasks and Eden notes. Before running them on real data, they need migration backups, structured reports, and partial-failure behavior so a single bad record does not abort app startup. The user also set a monorepo-wide invariant: all project tests must use test databases, never a main/user database.

## Acceptance Criteria

AC1. Root `AGENTS.md` documents that all tests and smoke checks must use isolated test databases and must not read/write a main/user ARK database.

AC2. Delphi task migration writes a one-per-version backup snapshot before migrating legacy todos when a backup directory is provided.

AC3. Delphi task migration returns and persists a structured report with migrated/skipped/failed counts, backup path, and per-record errors.

AC4. Delphi task migration continues past individual record failures and does not write the success migration marker when failures occur.

AC5. Eden note migration writes a one-per-version backup snapshot before migrating Heart note types/entries when a backup directory is provided.

AC6. Eden note migration returns a structured report with type/object/link counts, backup path, and per-record errors, while continuing past individual object/link failures.

AC7. Tests cover backup/report/partial-failure behavior without using a main/user database.

AC8. Fresh verification runs relevant Delphi/Eden tests, builds, diff checks, and records evidence.
