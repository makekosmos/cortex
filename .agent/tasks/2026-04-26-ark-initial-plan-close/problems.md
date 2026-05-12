# Problems

No unresolved problems.

## Resolved During Verification

- First `ark:smoke` run failed on Windows because Node attempted to spawn
  `bun.cmd` with `shell: false`. Fixed by using shell execution on Windows.
- Second `ark:smoke` run failed because `.cmd` suffix was no longer needed with
  shell execution. Fixed by invoking `bun` / `bunx` directly.
