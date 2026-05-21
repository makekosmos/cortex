# Problems

## Remaining blocker

- `playwright test` is blocked by the Codex desktop sandbox on this machine image. The failure happens after `globalSetup` succeeds and before any test body executes:
  - `Error: spawn EPERM`
  - `at WorkerHost.startRunner (...)`

## Why this is not a repo-code failure

- The smoke DB is seeded successfully by Python before Playwright starts workers.
- `tsc` and `vite build` both pass against the current code.
- The failing syscall is process spawn/fork inside the test runner, which is controlled by the execution environment rather than dashboard code.

## Local machine verification command

- `cd D:\Personal\Hobby\Coding\kosmos\apps\dashboard`
- `node D:\Personal\Hobby\Coding\kosmos\node_modules\.bun\playwright@1.58.2\node_modules\playwright\cli.js test`
