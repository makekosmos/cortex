# Problems

No unresolved problems.

## Resolved

- Delphi Playwright Electron launch initially failed before `app.ready` because the test overrode `USERPROFILE` on Windows. The e2e harness now leaves `USERPROFILE` untouched and passes explicit `KOSMOS_TEST_APPDATA` / `KOSMOS_TEST_USER_DATA` paths instead.
- Delphi Playwright initially read ARK SQLite directly with `better-sqlite3`, which hit a native ABI mismatch. The e2e now verifies persistence through the Delphi/Eden ARK IPC path and only checks the test DB file path with `fs.existsSync`.
