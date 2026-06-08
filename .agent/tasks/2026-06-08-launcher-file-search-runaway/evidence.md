# Evidence — 2026-06-08 launcher file search runaway

Verified at: 2026-06-08

## AC1 — Hide cancels file search

Verdict: PASS

Evidence:

- `platform/desktop/electron/main.ts::hideLauncher` sends `kepler:window:hide` before `BrowserWindow.hide()`.
- `platform/desktop/electron/preload.ts` exposes `window.kepler.window.onHide`.
- `platform/desktop/shared/ipc-types.ts` includes the typed `onHide` contract.
- `LauncherView.vue` subscribes to `onHide` and calls `cancelFileSearch()`, which increments `fileSearchRun`, clears the timer, nulls it, and clears `fileCommands`.

Command:

```powershell
bun test tests/unit/launcher-file-search-contract.test.ts
```

Result: 3 pass, 0 fail.

## AC2 — No infinite polling

Verdict: PASS

Evidence:

- Removed `FILE_SEARCH_REFRESH_MS`.
- `scheduleFileSearch` performs one debounced search and no longer schedules itself after success.
- `cancelFileSearch()` is also called when leaving command mode.

Command:

```powershell
bun test tests/unit/launcher-file-search-contract.test.ts
```

Result: 3 pass, 0 fail.

## AC3 — Short queries do not scan files

Verdict: PASS

Evidence:

- `FileStore::search` returns empty for `q.chars().count() < 3`.
- Removed the now-unused `search_like` substring path.
- Added `short_queries_do_not_scan_files_table`.
- Updated old file-index follow-up test to use a 3+ char query under the new contract.

Commands:

```powershell
$env:CARGO_TARGET_DIR='.tmp\cargo-file-search-test'; cargo test -p kepler-backend short_queries_do_not_scan_files_table
$env:CARGO_TARGET_DIR='.tmp\cargo-file-search-test'; cargo test -p kepler-backend rescan_schedules_followup_when_generation_changes_during_write
```

Result: both targeted tests passed.

## AC4 — Background throttling

Verdict: PASS

Evidence:

- `backgroundThrottling` is now `process.platform !== "darwin"`, keeping the macOS occlusion workaround while allowing throttling for hidden Windows/Linux launcher windows.

Command:

```powershell
bun test tests/unit/launcher-file-search-contract.test.ts
```

Result: 3 pass, 0 fail.

## AC5 — Guards

Verdict: PASS

Commands:

```powershell
bun run shell:typecheck
bun run ark:guard:writes
bun run docs:sync
bun run docs:check
$env:CARGO_TARGET_DIR='.tmp\cargo-ark-smoke-file-search'; bun run ark:smoke
```

Results:

- `shell:typecheck`: PASS.
- `ark:guard:writes`: PASS.
- `docs:sync`: PASS.
- `docs:check`: PASS.
- `ark:smoke`: PASS.

Notes:

- Several commands initially failed before execution with `windows sandbox: setup refresh failed with status exit code: 1`; each was rerun with scoped escalation per `windows-sandbox`.
- The first plain `ark:smoke` failed with `os error 5` removing `target\debug\ark-core-rpc.exe` because live dev processes were using the default target dir. The successful smoke used an isolated `.tmp\cargo-ark-smoke-file-search` target dir.
- One old file-index test failed after AC3 because it searched for a 1-character query. The assertion was updated to use `a.md`, preserving the rescan follow-up behavior under the new 3+ character search contract.
