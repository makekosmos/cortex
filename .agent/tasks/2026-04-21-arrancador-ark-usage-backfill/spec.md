# Spec: Arrancador backfills legacy usage history into Ark and switches to Ark-only usage

## Original task

User wants the historical playtime that Arrancador collected locally to be pushed into Ark itself. After that, Arrancador should stop relying on its own local usage collection and consume only Ark usage-tracker data.

## Findings that inform scope

- Existing Ark migration code only backfills `game_obj` records, not usage history.
- Legacy Arrancador usage history currently lives in:
  - `playtime_daily`
  - `games.total_playtime`
  - `games.last_played`
- Current read-model still knows how to merge/fallback to local legacy usage.
- Current launch flow still updates local `games.play_count` and `games.last_played`, which is a form of Arrancador-owned usage tracking.
- Ark usage history is stored in `tracked_apps` and `usage_sessions`, with sync visibility driven by `sync_kv.lan_sync.version_vector`.

## Scope

Implement a one-time legacy usage backfill from Arrancador local DB into the effective Ark usage DB, then switch Arrancador usage reads to Ark-only and stop local launch-based usage tracking updates.

## Assumptions

- Legacy daily totals are good enough to represent historical usage in Ark as synthetic sessions.
- When `games.total_playtime` exceeds the sum of `playtime_daily`, the remainder should be preserved as synthetic imported usage instead of being dropped.
- Reading from the effective usage DB may still need the selected-space/root fallback logic, because tracker data currently may live in the root Ark DB.

## Acceptance Criteria

- AC1: Arrancador can backfill legacy local usage history into the effective Ark usage DB as `tracked_apps` + `usage_sessions`.
- AC2: Backfill is idempotent and does not create duplicate imported sessions on repeated runs.
- AC3: After backfill, Arrancador usage hydration and statistics read only from Ark usage data, not from legacy local playtime tables.
- AC4: Launching a game no longer updates Arrancador-owned local usage fields such as `play_count` / `last_played`.
- AC5: Focused automated tests cover the backfill plan/idempotence and Ark-only read-model behavior.
- AC6: Fresh verification passes for the touched Arrancador TypeScript code.

## Constraints

- Preserve the existing selected-space/root fallback for locating the effective usage-tracker Ark DB.
- Do not remove the legacy local columns/tables from the schema in this task.
- Keep the public UI contracts stable unless a backend-only extension is clearly needed.

## Non-goals

- Reconstructing exact historical foreground events; synthetic sessions are acceptable.
- Migrating legacy usage into `usage_events`.
- Reworking UI around session counts.

## Verification plan

- Add focused backend tests for legacy-to-Ark import planning and Ark-only hydration/stat reads.
- Run `bun test electron/main/services/ark-usage.test.ts electron/main/services/ark-usage-backfill.test.ts`.
- Run `bun run test`.
- Run `bun run typecheck`.
- Run `bun run build:main`.
