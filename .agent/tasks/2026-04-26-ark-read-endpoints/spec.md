# Task: ARK read endpoints for Arrancador

## Context

Arrancador ARK writes now go through `@kepler/ark`, but several read-only paths still open Ark SQLite directly from Electron main. Dashboard intentionally remains a read-only inspector, but Arrancador should prefer ARK runtime/SDK reads for normal app behavior.

All verification must use test/smoke databases only, never a user/main database.

## Acceptance Criteria

- AC1: Add or expose ARK runtime/SDK read operations needed by Arrancador for object hydration and usage/process summaries.
- AC2: Convert suitable Arrancador read-only ARK paths from direct SQLite to `@kepler/ark` APIs while preserving read-only fallback behavior where runtime startup fails.
- AC3: Keep Dashboard documented as a read-only database inspector and do not convert it away from direct read-only SQLite in this task.
- AC4: Document the remaining legacy/Heart decisions and the questions that require product input.
- AC5: Fresh verification passes on the current workspace using only isolated test/smoke databases.
