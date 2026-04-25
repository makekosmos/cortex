# Arrancador Usage Backfill Via SDK

## Context

Arrancador legacy usage backfill writes `tracked_apps` and `usage_sessions` directly into Ark SQLite and manually bumps `lan_sync.version_vector`. After `ark-core-rpc` local writes were updated to record sync state, the entity write path can move to the Ark SDK.

## Scope

Move Arrancador usage backfill entity writes from direct SQL to `@arksync/node` usage API.

## Acceptance Criteria

- AC1: `backfillLegacyUsageToArk()` writes tracked apps through `usage.trackedApps.upsert`.
- AC2: `backfillLegacyUsageToArk()` writes sessions through `usage.sessions.upsert`.
- AC3: The backfill no longer manually writes `lan_sync.version_vector`.
- AC4: Existing marker/device-id `sync_kv` behavior remains intact.
- AC5: Idempotent forced reruns still skip existing sessions and avoid duplicating tracked apps.
- AC6: Fresh Arrancador TypeScript/tests/build/lint verification commands are recorded in this task directory and all acceptance criteria are `PASS`.

## Out Of Scope

- Replacing the backfill marker storage.
- Adding usage events to the legacy backfill.
- Rewriting read-only usage analytics.
