---
name: ark-test-env
description: Reset Ark test environment — re-seed DB with 70 clean tasks, restart server, rebuild & install Android APK. Use when testing sync across devices or when data has diverged.
user_invocable: true
---

# Ark Test Environment Reset

Skill for resetting the Ark sync test environment to a clean state. Useful when data has diverged between devices or when you need a fresh start for testing.

## What it does

1. Stops the running Ark server (port 8000)
2. Re-seeds `ark.db` with exactly 70 test tasks (10 per GTD type)
3. Starts the Ark server with a new `server_epoch`
4. Optionally rebuilds and installs Android APK

## Commands

### Full reset (seed + server + Android)

```bash
cd /Users/kirill/Documents/projects/kosmos/packages/ark

# Stop server, re-seed, restart
lsof -ti:8000 | xargs kill -9 2>/dev/null
sleep 1
LIFE_DB_PATH=ark.db python3 seed_delphi.py
LIFE_DB_PATH=ark.db LIFE_API_KEY=dev-test-key ARK_DEVICE_NAME=ark-nebula ARK_MDNS=0 \
  nohup .venv/bin/uvicorn server.app:app --host 0.0.0.0 --port 8000 > /tmp/ark-server.log 2>&1 &
sleep 2
curl -s http://localhost:8000/health
```

### Rebuild & install Android

```bash
cd /Users/kirill/Documents/projects/kosmos/apps/delphi/kotlin
./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

### Verify DB state

```bash
cd /Users/kirill/Documents/projects/kosmos/packages/ark
sqlite3 ark.db "SELECT source, COUNT(*) FROM events WHERE is_deleted=0 GROUP BY source"
# Expected: 70|delphi-seed
```

## Seed task distribution

| GTD Type | Count | Key fields |
|----------|-------|------------|
| Inbox | 10 | no project, not today/someday/scheduled |
| Today | 10 | `isToday=true` |
| Upcoming | 10 | `scheduledDate` = +7 days |
| Someday | 10 | `isSomeday=true` |
| Completed | 10 | `isCompleted=true`, `completedAt` set |
| Cancelled | 10 | `isCancelled=true`, `cancelledAt` set |
| Trashed | 10 | `isTrashed=true` |

## After reset: device cleanup

On each device, press **"Очистить данные"** (Clear data) in settings BEFORE connecting to the server. This resets the local DB, version vector, and server epoch so the device gets a clean full sync.

- **macOS (SwiftUI)**: Settings window > "Очистить данные"
- **Android**: Settings > "Очистить данные"
- **Electron**: Click colored dot (top-right) > "Очистить данные"

## Critical sync rules (learned the hard way)

1. **All UUIDs MUST be lowercase** across all platforms. Swift sends `.lowercased()`, TS/Kotlin normalize on receive.
2. **Server upserts by `source_id` alone** (not `(source, source_id)`) to prevent duplicates from different clients.
3. **`pushLocalTodos` / `sendMissingToServer`** only runs on `is_full_sync=true` or `epochChanged=true` — NEVER on incremental reconnect.
4. **Zombie cleanup** only when `is_full_sync=true AND epochChanged=false`.
5. **Never auto-reset version vectors** based on task count comparison — this is dangerous and was explicitly rejected.
