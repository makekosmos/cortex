# Evidence: Arrancador merge usage-tracker data with legacy local playtime history

## Current Status

PASS

## Delivered scope

- Arrancador no longer treats usage-tracker as a replacement for local legacy playtime history:
  - `apps/arrancador/electron/main/services/ark-usage.ts`
- Statistics now merge both sources at `game + day` granularity:
  - tracker rows override matching legacy rows for the same game/day
  - legacy rows remain for older or otherwise missing tracker days
- Hydrated game totals now preserve legacy cumulative totals while appending tracker-only days and promoting the newest `last_played` timestamp:
  - `apps/arrancador/electron/main/services/ark-usage.ts`
- Focused backend tests cover both the merged statistics path and the merged hydration path:
  - `apps/arrancador/electron/main/services/ark-usage.test.ts`

## Acceptance Criteria status

- AC1: PASS
  - `createPlaytimeStatsRepository()` now combines usage-tracker rows with `playtime_daily` instead of replacing legacy history.
- AC2: PASS
  - Merge is done per `game/day`, so overlapping tracker and legacy rows for the same game/date resolve to one source instead of double counting.
- AC3: PASS
  - `createGameUsageReadModel()` keeps legacy `total_playtime`, adds tracker-only day totals, and uses the latest timestamp between legacy and tracker `last_played`.
- AC4: PASS
  - Targeted automated tests were added for merged stats and merged game hydration.
- AC5: PASS
  - Fresh verification completed successfully for the touched Arrancador backend code.

## Verification run

- `bun test electron/main/services/ark-usage.test.ts` -> PASS
- `bun run test` -> PASS
- `bun run typecheck` -> PASS
- `bun run build:main` -> PASS

## Notes

- Legacy Arrancador history still lives in:
  - `games.total_playtime`
  - `games.last_played`
  - `playtime_daily`
- The current merge intentionally avoids obvious duplicate counting by preferring one source per `game/day`.
- I attempted an additional direct runtime check against the real local DBs, but `bun` on Windows cannot load `better-sqlite3` in this environment, and `tsx` is not installed for a `node --import tsx` fallback. Automated backend verification and focused unit coverage did pass.
