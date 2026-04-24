# Problems

## P1: Electron runtime verification is blocked by environment process restrictions

### Symptom

Both verification paths that attempt to launch Electron through Playwright fail with `spawn EPERM` before the application flow starts:

- `bun x playwright test e2e/shared-ark-task.spec.ts --reporter=line`
- `node scripts/verifySharedArkTask.mjs`

### Why this matters

This prevents a fresh end-to-end proof for:

- Delphi booting into the shared selected space at runtime
- Delphi creating a task through the UI
- the task appearing in the shared `ark.db`
- Eden opening that same task from the same selected space

### Smallest safe fix attempted

Added a direct Playwright API verification script (`scripts/verifySharedArkTask.mjs`) to avoid the Playwright worker-process limitation.

### Result

The workaround still fails because the environment blocks `electron.launch` itself with `spawn EPERM`.

### Required follow-up outside this run

Run the new Delphi/Eden verification on a machine or sandbox that allows Electron child-process launch. Recommended commands after builds:

1. `node apps/delphi/ts/scripts/verifySharedArkTask.mjs`
2. `playwright test apps/delphi/ts/e2e/shared-ark-task.spec.ts`
