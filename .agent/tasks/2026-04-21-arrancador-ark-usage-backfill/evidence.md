# Evidence: Arrancador backfills legacy usage into Ark and switches to Ark-only usage

## Current Status

PASS

## Delivered scope

- Added a dedicated legacy-usage backfill path from Arrancador local DB into Ark usage tables:
  - `apps/arrancador/electron/main/services/ark-usage-backfill.ts`
- Backfill imports legacy daily history and residual cumulative playtime into synthetic Ark `tracked_apps` + `usage_sessions` entries, updates `sync_kv.lan_sync.version_vector`, and stores a marker to avoid repeat imports on normal startup.
- Startup runtime now triggers the backfill before creating services:
  - `apps/arrancador/electron/main/backend.ts`
- Usage hydration and statistics were switched back to Ark-only reads:
  - `apps/arrancador/electron/main/services/ark-usage.ts`
- Local launch-based usage updates were removed from Arrancador:
  - `apps/arrancador/electron/main/services/games.ts`
- `game_obj` hydration no longer reintroduces stale usage aggregates over the Ark usage-tracker data:
  - `apps/arrancador/electron/main/services/ark-game-objects.ts`
- Focused backend tests were added/updated:
  - `apps/arrancador/electron/main/services/ark-usage-backfill.test.ts`
  - `apps/arrancador/electron/main/services/ark-usage.test.ts`

## Acceptance Criteria status

- AC1: PASS
  - Legacy local usage history can now be imported into the effective Ark usage DB as synthetic `tracked_apps` + `usage_sessions`.
- AC2: PASS
  - Backfill is idempotent via a stored marker on normal startup and deterministic session IDs on forced reruns.
- AC3: PASS
  - Arrancador statistics and usage hydration now read Ark usage data only; legacy local playtime is no longer used as a live fallback source.
- AC4: PASS
  - Launch flow no longer updates local `play_count` / `last_played`.
- AC5: PASS
  - Focused automated tests cover the import plan, rerun behavior, Ark-only stats, and Ark-only hydration.
- AC6: PASS
  - Fresh verification passed for the touched Arrancador TypeScript code.

## Verification run

- `bun test electron/main/services/ark-usage.test.ts electron/main/services/ark-usage-backfill.test.ts` -> PASS
- `bun run test` -> PASS
- `bun run typecheck` -> PASS
- `bun run build:main` -> PASS

## Notes

- I attempted to execute the live backfill directly against the user's `%AppData%` Ark DB from this session, but the desktop sandbox does not allow writing outside the workspace, so the direct runtime import could not be completed here.
- The code path is in place, and the import will run from Arrancador itself on the next application startup.
