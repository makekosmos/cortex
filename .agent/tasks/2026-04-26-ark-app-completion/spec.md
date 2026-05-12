# 2026-04-26 ARK app completion

## Scope

Finish the remaining application-level ARK work that can be completed safely in this repository:

- Arrancador must not write directly into ARK SQLite tables from Electron main services.
- Eden notes and typed notes must be treated as ARK generic objects (`note_obj`) with automatic startup migration behavior documented and covered by existing or added checks.
- Dashboard remains a read-only data inspector for ARK databases, similar to a database console; it may inspect SQLite directly but must not write.
- Add or document a practical smoke matrix that uses isolated test databases only.
- Refresh stale ARK docs/TODO entries so current integration boundaries are clear.

## Acceptance Criteria

AC1. Arrancador ARK write-path audit passes: any writes to ARK `objects`, `object_types`, `object_links`, or usage sync tables go through `@kepler/ark` APIs or Rust `ark_core` helpers, not raw `better-sqlite3` SQL in app services.

AC2. Arrancador still has tests covering game object migration/write behavior and usage backfill behavior on isolated test databases or mocked ARK APIs.

AC3. Eden note persistence is confirmed object-first: notes/typed notes use `note_obj` ARK objects, and automatic startup migration is either implemented or already covered by code/tests.

AC4. Dashboard read-only inspector boundary is documented: direct SQLite reads are allowed for inspection/analytics, direct writes are not.

AC5. A smoke/check matrix exists for the current ARK app integration surface and all listed checks use isolated test databases or temporary app data paths.

AC6. Stale ARK docs/TODO wording is updated so it no longer instructs developers to follow obsolete runtime paths.

AC7. Fresh verification runs complete and results are recorded in `evidence.md`, `evidence.json`, and raw artifacts.

## Out of Scope

- Changing Dashboard into an ARK-only API consumer.
- Work that requires manual testing against a user's main ARK database.
- Mobile platform changes.
