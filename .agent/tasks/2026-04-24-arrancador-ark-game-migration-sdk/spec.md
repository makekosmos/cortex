# Task: Arrancador Ark Game Migration Writes Through SDK

## Context

Arrancador game sync already writes Ark game objects through `@arksync/node`, but `ark-game-migration.ts` still opens the target Ark SQLite DB directly and writes `object_types` and `objects` with SQL. That bypasses the ARK runtime, typed SDK, and local sync state.

## Scope

- Keep the source Ark DB as a read-only migration input.
- Replace target Ark DB reads/writes in `ark-game-migration.ts` with `@arksync/node` object/object type APIs.
- Preserve existing merge behavior: match by object id, `arrancador_game_id`, or normalized `exe_path`; merge props; skip unchanged objects.
- Add an injectable target SDK API for tests and avoid spawning a sidecar in focused unit tests.
- Add focused tests for type migration, object migration, merge, skip, and direct-SQL write prevention.

## Acceptance Criteria

- AC1: `ark-game-migration.ts` no longer writes target `object_types` or `objects` via direct SQL or target transactions.
- AC2: Target `game_obj` type creation uses `ArkClient.objectTypes.upsert` or an injected equivalent.
- AC3: Target game object migration/merge uses `ArkClient.objects.upsert` or an injected equivalent.
- AC4: Existing merge semantics are preserved for new, matched, and unchanged game objects.
- AC5: Fresh verification passes: Arrancador typecheck, focused migration tests, build:main, focused Biome check, no-direct-write grep, and `git diff --check`.
