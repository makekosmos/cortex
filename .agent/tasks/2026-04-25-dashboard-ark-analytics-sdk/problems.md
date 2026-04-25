# Verification Problems

## P1: Dashboard build needs unsandboxed process spawning

The first `bun run --cwd apps/dashboard build` failed inside the sandbox during `electron-rebuild -f -w better-sqlite3` with `spawn EPERM`. Re-running the same build outside the sandbox succeeded. This is an environment permission issue, not a code failure.
