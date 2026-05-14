# Evidence: Arrancador usage tracker fallback for missing selected-space sessions

## Current Status

PASS

## Delivered scope

- Usage/stat reading no longer assumes that tracker rows live in the selected-space Ark DB only:
  - `apps/arrancador/electron/main/services/ark-usage.ts`
- Arrancador now resolves an effective usage-tracker DB by:
  - preferring the selected-space Ark DB when it already has `usage_sessions`
  - falling back to the root `Roaming\\Kosmos\\ark.db` when the selected-space DB has no tracker rows
- Selected-space game object sync remains unchanged:
  - `apps/arrancador/electron/main/backend.ts`
  - `apps/arrancador/electron/main/services/ark-game-objects.ts`
- The playtime repository wrapper now forwards the fallback path:
  - `apps/arrancador/electron/main/services/playtime-stats.ts`
- Focused backend tests were added for fallback selection:
  - `apps/arrancador/electron/main/services/ark-usage.test.ts`

## Local environment facts confirmed during investigation

- Shared selected space:
  - `spaceId = f028287f78de2d7e`
- Selected-space Ark DB:
  - `C:\\Users\\Kazui\\AppData\\Roaming\\Kosmos\\spaces\\f028287f78de2d7e\\ark.db`
  - `usage_sessions = 0`
- Root Ark DB:
  - `C:\\Users\\Kazui\\AppData\\Roaming\\Kosmos\\ark.db`
  - `usage_sessions = 1609`
  - contains tracked app and usage rows for `VALORANT`
- Arrancador library entry for `VALORANT` was present, but its stored `last_played` stopped at `2026-04-16T19:54:26.571592100+00:00`, which matches the missing fallback behavior.

## Acceptance Criteria status

- AC1: PASS
  - Usage read-model now resolves tracker DB independently from selected-space object sync DB.
- AC2: PASS
  - Fallback to root Ark DB is used when the selected-space Ark DB has no usage tracker rows.
- AC3: PASS
  - Selected-space Ark DB remains primary when it does contain tracker rows.
- AC4: PASS
  - Focused backend tests passed and Arrancador verification passed for touched code.

## Verification run

- `bun run test` -> PASS
- `bun test electron/main/ark-runtime.test.ts electron/main/services/ark-usage.test.ts` -> PASS
- `bun run typecheck` -> PASS
- `bun run build:main` -> PASS

## Notes

- The fallback fix restores access to tracker data that already exists in the root Ark DB.
- The current local tracker data itself does not contain sessions on `2026-04-20` or `2026-04-21`; the latest recorded usage in the root Ark DB is `2026-04-17T23:33:37.201Z`.
