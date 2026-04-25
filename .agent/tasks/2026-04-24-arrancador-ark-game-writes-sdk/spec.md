# Arrancador Ark Game Writes Via SDK

## Context

Arrancador currently syncs games to Ark by opening Ark SQLite directly and writing rows into `objects`. That bypasses the Ark runtime API, typed request handling, sidecar lifecycle, and future sync journal behavior.

## Scope

Move the Arrancador game object write path from direct SQLite writes to the `@arksync/node` object API backed by `ark-core-rpc`.

## Acceptance Criteria

- AC1: `createArkGameObjectService().syncGame()` writes game objects via `ArkClient.objects.upsert` / `upsert_object`, not by executing `INSERT OR REPLACE INTO objects`.
- AC2: `syncGame()` still reuses an existing object by `ark_object_id`, `arrancador_game_id`, or normalized `exe_path`.
- AC3: Self-managed Arrancador runtime can resolve and package `ark-core-rpc` for dev and packaged builds.
- AC4: Arrancador declares the `@arksync/node` workspace dependency.
- AC5: Existing read-only hydration behavior remains supported.
- AC6: Fresh TypeScript/tests/build verification commands are recorded in this task directory and all acceptance criteria are `PASS`.

## Out Of Scope

- Converting Arrancador read-only usage analytics from SQL to Ark analytics endpoints.
- Rewriting `ark-game-migration.ts`.
- Converting usage backfill writes to SDK calls.
- Removing `better-sqlite3` from Arrancador.
