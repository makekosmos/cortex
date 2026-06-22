# Windows Cargo Target Lock

## Trigger

`cargo test`, `cargo build`, or `cargo fmt` fails on Windows with `failed to open ... target\debug\.cargo-lock` or `failed to remove file ... target\debug\*.exe` and `Access is denied`.

## Symptom

The workspace build starts, then Cargo exits before running tests because it cannot open the shared target lock file or replace a binary still held by a running dev backend.

## Do This

First check whether a running dev process owns the workspace target executable:

```powershell
Get-Process | Where-Object { $_.ProcessName -like "*kepler*" -or $_.ProcessName -like "*kosmos*" -or $_.ProcessName -like "*stt*" } | Select-Object Id,ProcessName,Path
```

If the user is actively running dev, do not kill it just to test. Use a separate target dir:

```powershell
$env:CARGO_TARGET_DIR='target/codex-check'; cargo test --manifest-path platform/runtime/Cargo.toml dictation::host
```

If the process is a stale binary from your own aborted command, stop only that specific PID and rerun.

## Avoid

Do not treat the lock denial as a Rust test failure. Do not kill the user's live dev app unless they asked you to restart it.

## Promote To Skill When

This keeps happening across multiple Windows workspace repos or needs a more general command recipe.
