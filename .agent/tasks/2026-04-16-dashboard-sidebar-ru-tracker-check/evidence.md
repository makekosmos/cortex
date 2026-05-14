# Evidence

## Verification summary

- AC1 `PASS`: `apps/dashboard/src/components/dashboard/DashboardShell.vue` now uses the Delphi/Eden `KosmosSidebar` pattern with persisted `SidebarConfig`, `drag-region`, and a non-scrolling left rail while the content area owns scrolling.
- AC2 `PASS`: User-facing dashboard copy was translated to Russian across shell, overview, sessions, top apps, recent sessions, charts, heatmap titles, and formatting helpers.
- AC3 `PASS`: Dashboard styling continues to derive from `@kosmos/visuals` theme variables and keeps body scroll disabled so the sidebar remains visually fixed.
- AC4 `PASS`: Renderer and Electron bundles build successfully from the current repository checkout.
- AC5 `PASS`: The installed `usage-tracker` process is running and writing live usage rows into `%APPDATA%\\Kosmos\\ark.db`.

## Commands

```powershell
Set-Location D:\Personal\Hobby\Coding\kosmos\apps\dashboard; .\node_modules\.bin\tsc.exe
Set-Location D:\Personal\Hobby\Coding\kosmos\apps\dashboard; .\node_modules\.bin\vite.exe build --configLoader native
Get-Process | Where-Object { $_.ProcessName -like 'usage-tracker*' } | Select-Object ProcessName, Id, StartTime, Path | Format-List
@'
import os, sqlite3, json
path = os.path.join(os.environ['APPDATA'], 'Kosmos', 'ark.db')
conn = sqlite3.connect(path)
cur = conn.cursor()
counts = {table: cur.execute(f'SELECT COUNT(*) FROM {table}').fetchone()[0] for table in ('tracked_apps','usage_sessions','usage_events')}
latest_session = cur.execute('SELECT process_name, window_title, started_at, ended_at, foreground_ms, idle_ms FROM usage_sessions ORDER BY started_at DESC LIMIT 1').fetchone()
latest_event = cur.execute('SELECT kind, occurred_at, process_name, window_title, is_idle FROM usage_events ORDER BY occurred_at DESC LIMIT 1').fetchone()
print(json.dumps({"db": path, "counts": counts, "latest_session": latest_session, "latest_event": latest_event}, ensure_ascii=False, indent=2))
'@ | python -
```

## Results

- `tsc.exe`: passed with exit code `0`
- `vite build --configLoader native`: passed with exit code `0`
- `usage-tracker.exe`: active as PID `37500`, path `C:\Users\Kazui\AppData\Local\Kosmos\UsageTracker\usage-tracker.exe`
- Live DB: `C:\Users\Kazui\AppData\Roaming\Kosmos\ark.db`
- Live counts at verification time:
  - `tracked_apps = 6`
  - `usage_sessions = 37`
  - `usage_events = 86`
- Latest rows at verification time:
  - latest session: `Codex.exe`, window `Codex`, `foreground_ms=65000`, `idle_ms=250000`, `ended_at=NULL`
  - latest event: `idle_started` for `Codex.exe`

## Notes

- The tracker is clearly active and writing, but timing precision is still poll-based. Current default polling in `services/usage-tracker/src/main.rs` is `5000 ms`, so foreground and idle totals are quantized to roughly 5-second slices.
