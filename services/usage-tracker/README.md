# usage-tracker

Windows-first background executable that tracks the current foreground application
and writes usage data directly into Ark DB.

## What it writes

- `tracked_apps`
- `usage_sessions`
- `usage_events`

It also updates Ark's `lan_sync.version_vector` so these direct DB writes remain
visible to the Ark sync layer on later sync passes.

The process is intentionally quiet:

- no UI
- no tray icon
- no Windows Service wrapper
- one user-level background process with a small polling loop

## Installer bundle

Build a release install bundle:

```powershell
bun run package:installer
```

This produces `dist/KeplerUsageTrackerInstaller/` with:

- `usage-tracker.exe`
- `install.ps1`
- `uninstall.ps1`
- `Install Usage Tracker.cmd`
- `Uninstall Usage Tracker.cmd`
- `manifest.json`

It also creates:

- `dist/KeplerUsageTrackerInstaller.zip`

Default install target:

- `%LOCALAPPDATA%\\Kepler\\UsageTracker`

Default install behavior:

- copy the release binary into the install directory
- register autostart via `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`
- launch the tracker immediately in the background

Useful installer flags:

- `install.ps1 -NoStartup`
- `install.ps1 -NoLaunch`
- `install.ps1 -InstallDir D:\\Somewhere\\UsageTracker`

If you want a single-click entrypoint from Explorer, use:

- `Install Usage Tracker.cmd`
- `Uninstall Usage Tracker.cmd`

## Defaults

- Ark DB path: `%APPDATA%\\Kepler\\ark.db`
- Poll interval: `5000` ms
- Idle threshold: `60` s

Automated tests and smoke checks must not use the default user DB path. For
checks, set `ARK_DB_PATH` or pass `--db-path` to a database under `.tmp`,
`.agent/tasks/<TASK_ID>/`, `.e2e`, or an OS temp directory.

## Overrides

- `ARK_DB_PATH`
- `USAGE_TRACKER_POLL_MS`
- `USAGE_TRACKER_IDLE_SECS`
- `USAGE_TRACKER_RUN_ONCE=1`

CLI flags override the defaults and env values:

- `--db-path <path>`
- `--poll-ms <number>`
- `--idle-secs <number>`
- `--once`

## Notes

- This is a normal user-level background executable, not a Windows Service.
- `Arrancador` should consume the resulting Ark usage data instead of launching
  or owning this process.
- `v1` is Windows only; a later macOS backend should plug in behind the same
  capture/persistence split.
- `tracked_apps` are updated on session boundaries; `usage_sessions` and
  `usage_events` carry the fine-grained usage stream.
