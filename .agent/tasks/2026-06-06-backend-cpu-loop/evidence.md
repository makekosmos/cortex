# Evidence — 2026-06-06 backend CPU loop

## Pre-fix runtime evidence

- User-visible 30s delta before changes:
  - `kepler-backend.exe` PID 28132: `27.81 CPU sec / 30s` = `92.7%` of one core.
  - `ark-core-rpc.exe` PID 21820: `14.03 CPU sec / 30s` = `46.8%` of one core.
  - RAM was stable (`kepler-backend` private `19.3 MB -> 19.4 MB`), so this was CPU churn, not a memory leak.
- Backend log showed file index startup scan:
  - `file_index initial rescan total=230691 roots=2 ...`
- Read-only aggregate DB probe over 10s:
  - `usage_sessions WHERE ended_at IS NULL`: `SUM(runtime_ms)` grew from `295825750` to `295840924`.
  - `lan_sync.version_vector` length stayed `2066991`, hash changed `be3142a77205 -> f76e0982de36`.
  - This proved repeated upserts of existing sync entities.
- After usage heartbeat throttling but before watcher fix:
  - Live post-fix profile still showed `kepler-backend` `29.39 CPU sec / 30s` (`98.0%` one core) while `ark-core-rpc` dropped to `2.62 CPU sec / 30s` (`8.8%` one core).
  - Isolated backend with empty file-index roots and usage tracker enabled showed `kepler-backend` `0.06 CPU sec / 30s` (`0.2%`) and `ark-core-rpc` `0.00`.
  - This isolated the remaining backend CPU to recursive file-index watcher on persisted broad roots, not ARK writes.

## AC Results

**AC1 — PASS.** Root cause identified in concrete paths:

- `services/kepler-backend/src/usage_tracker/mod.rs::run`: per-poll session/tracked-app persistence.
- `services/kepler-backend/src/file_index/watcher.rs::start`: recursive watcher enabled by default for persisted roots.

**AC2 — PASS.** Fixes:

- Usage session heartbeat is bounded to `SESSION_HEARTBEAT_FLUSH_MS = 60_000` and final session persistence remains intact.
- Heartbeat persists only `usage_session`; `tracked_app.last_seen_at` is persisted at session finalization to avoid a second sync/version-vector bump per checkpoint.
- Recursive file watcher is opt-in via `KEPLER_FILE_INDEX_WATCHER=1`; default path is startup/manual rescan.

**AC3 — PASS.** Regression tests:

```powershell
$env:CARGO_TARGET_DIR='.tmp\cargo-usage-cpu-test'; cargo test -p kepler-backend active_session_heartbeat_flush_is_rate_limited -- --nocapture
```

Result: `1 passed`.

```powershell
$env:CARGO_TARGET_DIR='.tmp\cargo-usage-cpu-test'; cargo test -p kepler-backend recursive_watcher_is_opt_in -- --nocapture
```

Result: `1 passed`.

**AC4 — PASS.** Data/write boundary:

```powershell
bun run ark:guard:writes
```

Result: `ARK write boundary guard passed.`

**AC5 — PASS.** Postmortem entry added:

- `docs-site/agents/postmortems.md` entry `2026-06-06 — Backend жёг CPU usage heartbeat и file watcher'ом`.
- `bun run docs:sync` passed.
- `bun run docs:check` passed.

**AC6 — PASS.** Fresh fixed runtime measurement:

Fixed isolated backend from `.tmp\cargo-usage-cpu-test\debug\kepler-backend.exe`, usage tracker enabled, `KEPLER_SKIP_SYNC=1`, small explicit file-index root, watcher default disabled:

- `kepler-backend.exe` PID 3868: `0.08 CPU sec / 30s` = `0.3%` of one core.
- `ark-core-rpc.exe` PID 27332: `0.02 CPU sec / 30s` = `0.1%` of one core.
- RAM stable: backend `PrivateMB 10.1`, ARK `PrivateMB 2.3`.

Clean `bun run --cwd shell dev` after the second heartbeat tuning:

- `kepler-backend.exe` PID 1696: `0.00 CPU sec / 70s` = `0.0%` of one core.
- `ark-core-rpc.exe` PID 31268: `0.00 CPU sec / 70s` = `0.0%` of one core.
- RAM stable: backend `WorkingSetMB 43.1`, ARK `WorkingSetMB 15.3`.

**Substantial smoke — PASS.**

```powershell
bun run ark:smoke
```

Result: `ARK smoke matrix passed.`

## Notes

- Default `target\debug\kepler-backend.exe` rebuild failed while live user backend held the exe (`os error 5`). Verification therefore used isolated Cargo target dir `.tmp\cargo-usage-cpu-test`.
- A final process check found no running `kepler-backend` / `ark-core-rpc` processes, so no old hot backend remained alive after verification.
