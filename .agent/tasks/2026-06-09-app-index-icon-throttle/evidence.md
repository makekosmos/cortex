# Evidence — 2026-06-09 app-index-icon-throttle

## AC1 — no eager extraction in Windows sources

Verdict: PASS.

Command:

```powershell
rtk grep "ensure_icon_for_lnk|ensure_icon_for_uwp\(" platform/runtime/src/app_index/platform/windows platform/runtime/src/app_index/icons.rs
```

Result: matches exist only in `platform/runtime/src/app_index/icons.rs`; no matches under `platform/runtime/src/app_index/platform/windows/*`.

## AC2 — icon_source metadata

Verdict: PASS.

Evidence:

- `platform/runtime/src/app_index/app.rs` defines `IconSource::{StartMenuLnk,UwpPackage}`.
- `platform/runtime/src/app_index/platform/windows/start_menu.rs` stores `.lnk` path and target path with `icon_path: None`.
- `platform/runtime/src/app_index/platform/windows/uwp.rs` stores package full name with `icon_path: None`.

## AC3 — serial throttled extraction path

Verdict: PASS.

Evidence:

- `platform/runtime/src/app_index/mod.rs` uses `IconExtractionQueue::fill_missing_icons()` inside the existing `spawn_blocking` background-priority task.
- Queue iterates apps serially, concurrency=1.
- Queue sleeps after cold extraction attempts and skips sleep for cache hits.

## AC4 — additive SQLite migration and round-trip

Verdict: PASS.

Command:

```powershell
$env:CARGO_TARGET_DIR='.tmp/cargo-app-icon-test'; rtk cargo test -p kepler-backend app_index
```

Result: `4 passed, 343 filtered out`.

## AC5 — guards

Verdict: PASS.

Commands:

```powershell
rtk cargo fmt -p kepler-backend
$env:CARGO_TARGET_DIR='.tmp/cargo-app-icon-test'; rtk cargo test -p kepler-backend app_index
rtk node scripts/check-ark-write-boundaries.mjs
rtk node scripts/sync-agents-docs.mjs
rtk node scripts/check-docs-freshness.mjs
$env:CARGO_TARGET_DIR='.tmp/cargo-ark-smoke-app-icons'; rtk node scripts/ark-smoke.mjs
```

Results:

- `cargo fmt` passed.
- app_index tests passed: `4 passed, 343 filtered out`.
- ARK write boundary guard passed.
- docs sync completed.
- docs check passed: stale references not found.
- ARK smoke matrix passed.

Notes:

- `bun run docs:sync` and `bun run ark:guard:writes` unexpectedly reported `Script not found` despite scripts existing in `package.json`, so equivalent direct `node scripts/...` commands were used.
- Some Cargo commands needed `CARGO_TARGET_DIR=.tmp/...` because the default `target/debug/.cargo-lock` returned Windows `EPERM`.
