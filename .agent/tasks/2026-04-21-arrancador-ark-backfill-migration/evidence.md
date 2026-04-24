# Evidence

## Result

- Verification status: `PASS`
- Mojibake check: `PASS`

## Acceptance Criteria

- AC1: `PASS`
  Existing local Arrancador games are now bulk-synced through `syncAllGamesToArk()` in [games.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/services/games.ts:534), and the live local DB shows `17` games with `0` missing `ark_object_id`.
- AC2: `PASS`
  Startup backfill is triggered after the first renderer load in [index.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/index.ts:160) via [backend.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/backend.ts:776).
- AC3: `PASS`
  Manual sync is exposed through IPC/API with `sync_games_to_ark` in [backend.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/backend.ts:380), [ipc.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/types/ipc.ts:38), and [api.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/src/lib/api.ts:46).
- AC4: `PASS`
  Ark-to-Ark `game_obj` migration exists in [ark-game-migration.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/services/ark-game-migration.ts:201) and is exposed through `migrate_ark_games` IPC in [backend.ts](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/electron/main/backend.ts:381).
- AC5: `PASS`
  CLI/script migration is available through `bun run migrate:ark-games -- <source> [target]`, backed by [migrate_ark_games.py](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/scripts/migrate_ark_games.py:207) and [package.json](/D:/Personal/Hobby/Coding/kepler/apps/arrancador/package.json:11).
- AC6: `PASS`
  `bun run typecheck`, `bun run build:main`, and `python -m py_compile .\\scripts\\migrate_ark_games.py` all passed.

## Commands

1. `bun run typecheck`
   Passed.
2. `bun run build:main`
   Passed.
3. `python -m py_compile .\\scripts\\migrate_ark_games.py`
   Passed.
4. `bun run migrate:ark-games -- "<source-ark.db>" "<target-ark.db>"`
   Passed with result:
   `migratedTypes=1`, `migratedObjects=1`, `mergedObjects=0`, `skippedObjects=0`.
5. Live DB verification via Python `sqlite3`
   Current selected Ark DB contains `17` active `game_obj` rows.
   Local Arrancador DB contains `17` games and `0` missing `ark_object_id`.

## Raw Artifacts

- [source-ark.db](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-21-arrancador-ark-backfill-migration/artifacts/source-ark.db)
- [target-ark.db](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-21-arrancador-ark-backfill-migration/artifacts/target-ark.db)
