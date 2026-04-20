# Task: Arrancador Ark Game Object Sync

## Goal

Add bounded Arrancador-side support for linking library games to Ark `game_obj` records without touching Eden code:

- persist an `ark_object_id` alongside Arrancador games,
- read Ark `game_obj` data back into Arrancador game responses when linked,
- upsert Ark `game_obj` records from Arrancador write paths where practical in Electron main,
- avoid invasive frontend changes or Eden-specific coupling.

## Scope

In scope:

- `apps/arrancador/electron/main/db/*`
- `apps/arrancador/electron/main/services/*`
- minimal `apps/arrancador/electron/main/backend.ts` wiring if required
- task artifacts under this task directory

Out of scope:

- changes in `apps/eden`
- broad UI work in Arrancador renderer
- schema changes in Ark core
- full bi-directional conflict resolution semantics

## Acceptance Criteria

- AC1: Arrancador `games` storage supports a nullable `ark_object_id` and migrates existing DBs safely.
- AC2: Electron main game read models can hydrate linked games from Ark `game_obj` records on a best-effort basis without breaking when Ark DB is absent or malformed.
- AC3: Electron main game write paths upsert a corresponding Ark `game_obj` when Ark is available and persist the resulting `ark_object_id` locally.
- AC4: The patch remains bounded to Arrancador Electron/db/services/backend code and does not modify Eden.
- AC5: Verification documents compile/runtime risks and any unresolved sync semantics that remain outside the bounded patch.

## Notes

- Prefer direct Ark SQLite access already used by Arrancador usage read paths instead of introducing new cross-app runtime dependencies.
- Treat Ark sync as best-effort: Arrancador must continue working when Ark DB is missing, locked, or lacks the object tables.
