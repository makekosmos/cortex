# Spec: Arrancador merge usage-tracker data with legacy local playtime history

## Original task

User confirmed that Arrancador now pulls data from usage-tracker, and wants those stats to also include the historical playtime that Arrancador had already collected locally before the tracker integration.

## Findings that inform scope

- Arrancador still stores legacy playtime history in its local SQLite DB:
  - cumulative totals on `games.total_playtime`
  - last local session timestamp on `games.last_played`
  - daily history in `playtime_daily`
- The current Ark-backed read model treats usage-tracker as a replacement source:
  - stats prefer Ark rows and only fall back to legacy when Ark returns nothing
  - game hydration overwrites `total_playtime` and `last_played` with Ark aggregates
- In the local user DB, historical legacy totals are materially larger than the new tracker totals, so replacement loses history instead of extending it.

## Scope

Merge usage-tracker data with Arrancador legacy local playtime history so statistics and hydrated game totals preserve historical data while still including new tracker sessions.

## Assumptions

- Legacy local playtime is still authoritative for the pre-tracker era.
- Tracker and legacy data can overlap around the transition date, so merge rules must avoid obvious double counting.
- A conservative merge is better than replacing local history with tracker-only values.

## Acceptance Criteria

- AC1: Statistics read-model returns combined playtime from usage-tracker and legacy Arrancador history instead of replacing legacy data.
- AC2: When tracker and legacy history overlap for the same game/day, merge logic avoids duplicate counting by preferring a single source for that game/day.
- AC3: Hydrated game `total_playtime` and `last_played` preserve legacy totals while still incorporating newer tracker sessions.
- AC4: Focused automated tests cover the merge behavior.
- AC5: Fresh verification passes for the touched Arrancador backend code.

## Constraints

- Keep the usage-tracker fallback behavior from the previous fix.
- Do not require migrating or rewriting the user DB.
- Keep the public IPC/stat contracts unchanged.

## Non-goals

- Backfilling legacy history into Ark.
- Reworking the statistics UI.
- Designing a conflict-resolution UI for overlapping history.

## Verification plan

- Add targeted tests for merged daily totals, merged per-game totals, and hydrated game totals.
- Run `bun test electron/main/services/ark-usage.test.ts`.
- Run `bun run test`.
- Run `bun run typecheck`.
- Run `bun run build:main`.
