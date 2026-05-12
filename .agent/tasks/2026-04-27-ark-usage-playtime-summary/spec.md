# Task: ARK usage playtime summary endpoint

## Context

Arrancador already writes game objects and usage imports through ARK APIs, but
the remaining playtime hydration and statistics path still falls back to app-side
SQLite reads over ARK usage tables. That keeps ARK table knowledge in the
Electron main service and makes `@kepler/ark` less useful as the stable runtime
API.

All automated verification must use isolated test/smoke databases only. No test,
smoke, or migration verification may target a main/user ARK database.

## Scope

Move Arrancador usage aggregate reads to a real `ark-core-rpc` operation exposed
through `@kepler/ark`. Arrancador may still build the app-specific binding list
from its current game records, but ARK must own the usage-table aggregation.

## Acceptance Criteria

- AC1: `ark-core-rpc` exposes a playtime summary operation that accepts
  game/process bindings plus an optional date range and returns per-game usage
  aggregates and daily totals.
- AC2: `@kepler/ark` exposes a typed SDK method for the playtime summary
  operation.
- AC3: Arrancador `createGameUsageReadModel` hydrates `play_count`,
  `total_playtime`, and `last_played` through the new SDK method before any
  read-only SQLite fallback.
- AC4: Arrancador `createPlaytimeStatsRepository` uses the new SDK method for
  range stats before any read-only SQLite fallback.
- AC5: Existing direct read-only SQLite fallback remains available only as a
  runtime compatibility fallback and is not the primary path.
- AC6: Docs mention the new endpoint and keep the test database isolation rule.
- AC7: Fresh verification passes on the current workspace using only isolated
  test/smoke databases.
