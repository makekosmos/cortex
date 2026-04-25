# Verification Problems

## P1: PowerShell pipeline reported successful native commands as failures

The initial verification used `2>&1 | Tee-Object` for `cargo` and `bun` commands. PowerShell surfaced stderr/status lines as `NativeCommandError`, causing the tool call to return exit code 1 even though `cargo check` printed `Finished ...` successfully. Re-run verification with a wrapper that redirects all streams to files first, captures `$LASTEXITCODE`, prints the captured output, and exits with the actual native command status.

## P2: Full `git diff --check` includes unrelated pre-existing whitespace

The full worktree `git diff --check` reported `apps/arrancador/src-vue/pages/GameDetailPage.vue:392: new blank line at EOF.` The smallest safe fix was to remove that final blank line only; no UI behavior or content changed.

## P3: Full Rust test exposed test-level global DB race

`cargo test` initially failed because the two local-write RPC tests both call `Init` on the process-global `DB` and Rust runs tests in parallel. The production code was not changed for this; the tests that use the global DB are now serialized with a test-only `tokio::sync::Mutex`.
