# Evidence — 2026-06-08 file index startup scan safety

Verified at: 2026-06-08

## AC1

**Verdict: PASS.**

`platform/runtime/src/main.rs` now gates initial file index rescan on:

- `KEPLER_FILE_INDEX` enabled.
- `KEPLER_FILE_INDEX_INITIAL_RESCAN` enabled.
- `file_index.has_roots()?`.

With `KEPLER_FILE_INDEX_INITIAL_RESCAN=0`, the initial rescan branch is skipped before `tokio::spawn`.

## AC2

**Verdict: PASS.**

Covered by Rust regression:

```powershell
$env:CARGO_TARGET_DIR='.tmp/cargo-file-index-test'; cargo test -p kepler-backend file_index
```

Relevant passing tests:

- `file_index::scanner::default_roots_tests::production_default_roots_are_empty_without_opt_in`
- `file_index::scanner::default_roots_tests::env_override_takes_precedence`

## AC3

**Verdict: PASS.**

`FileIndex::rescan_locked()` now wraps `scanner::scan_roots_with_progress(...)` and `store.replace_all(&files)` in `tokio::task::spawn_blocking`.

## AC4

**Verdict: PASS.**

Covered by Rust regression:

```powershell
$env:CARGO_TARGET_DIR='.tmp/cargo-file-index-test'; cargo test -p kepler-backend file_index
```

Relevant passing test:

- `file_index::scanner::default_roots_tests::scan_walk_stops_when_cancelled`

The scanner checks cancellation inside the walk loop and stops before indexing all 1,000 synthetic files.

## AC5

**Verdict: PASS.**

Covered by Rust regression:

```powershell
$env:CARGO_TARGET_DIR='.tmp/cargo-file-index-test'; cargo test -p kepler-backend file_index
```

Relevant passing tests:

- `file_index::tests::disabled_index_exposes_empty_safe_surface`
- `file_index::tests::env_flag_parser_treats_zero_false_off_no_as_disabled`

`FileIndex::new_disabled()` keeps roots empty, search empty, and rescan stats at zero even when the DB contains persisted roots/files.

## AC6

**Verdict: PASS.**

Commands:

```powershell
$env:CARGO_TARGET_DIR='.tmp/cargo-file-index-test'; cargo test -p kepler-backend file_index
node scripts/check-docs-freshness.mjs
$env:CARGO_TARGET_DIR='.tmp/cargo-ark-smoke'; node scripts/ark-smoke.mjs
```

Results:

- `cargo test -p kepler-backend file_index`: 32 passed.
- `docs:check`: passed, no stale references.
- `ark:smoke`: passed. The first default-target attempts failed because live dev processes held `target/debug/kepler-backend.exe` / `ark-core-rpc.exe`; the successful verifier used isolated `.tmp` Cargo target dirs to avoid touching the live dev stack.
