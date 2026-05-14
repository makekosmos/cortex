# Spec: Arrancador usage tracker fallback for missing selected-space sessions

## Original task

User reports that Arrancador statistics do not show recent play sessions, specifically recent VALORANT play from the previous day. The user is unsure whether the usage tracker is enabled at all.

## Findings that inform scope

- The current shared selected space points Arrancador to a space-specific Ark DB.
- That selected-space Ark DB exists but its `tracked_apps` and `usage_sessions` tables are empty.
- The legacy/root Ark DB at `Roaming\\Kosmos\\ark.db` contains active usage tracker data, including recent VALORANT sessions.
- Arrancador currently uses the selected-space Ark DB for both game object sync and usage-stat reads.

## Scope

Fix Arrancador so usage-stat reads can fall back to the root Ark DB when the selected-space Ark DB has no tracked app/session data, without breaking selected-space game object sync.

## Assumptions

- Usage tracker rollout/migration is incomplete in the user environment, so root Ark DB still contains the live tracker data.
- Selected-space object sync should remain unchanged.
- We should implement a conservative fallback only for usage/stat reads, not a broad DB-path rewrite.

## Acceptance Criteria

- AC1: Arrancador usage read-model can resolve an effective usage-tracker DB path independently from the selected-space object DB path.
- AC2: When the selected-space Ark DB has no usage tracker rows, usage hydration and playtime statistics fall back to the root Ark DB.
- AC3: If the selected-space Ark DB does contain tracker rows, it remains the primary source.
- AC4: Fresh tests cover the fallback behavior and Arrancador verification passes for touched code.

## Constraints

- Do not change the selected-space game-object sync destination.
- Keep fallback logic localized to usage/stat reading paths.
- Do not require manual DB migration for this fix to work.

## Non-goals

- Migrating tracker rows between Ark DBs.
- Redesigning the shared-space selection system.
- Adding full diagnostics UI for tracker source selection.

## Verification plan

- Add focused tests for the usage-tracker fallback selection logic.
- Run `bun run test` in `apps/arrancador`.
- Run `bun run typecheck` in `apps/arrancador`.
- Run `bun run build:main` in `apps/arrancador`.
