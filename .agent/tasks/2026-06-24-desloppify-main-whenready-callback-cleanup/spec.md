# 2026-06-24 Desloppify Main WhenReady Callback Cleanup

## Classification

FULL_LOOP.

## Goal

Remove the `CALLBACK_PROMISE_MIX` finding from Electron main startup without
changing startup ordering or side effects.

## Context

`platform/desktop/electron/main.ts` used `app.whenReady().then(async () => { ... })`.
The body is now a named async function passed to `then`, keeping the same
`app.whenReady()` sequencing while avoiding an inline async callback.

## Scope

In scope:

- `platform/desktop/electron/main.ts`
- This task's evidence files

Out of scope:

- Further Electron main splitting
- Environment config centralization
- Startup behavior changes

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Startup behavior is preserved.**
The previous `whenReady` body still runs after `app.whenReady()` resolves.

**AC3. Relevant checks pass.**
Typecheck, shell build, and the full scan run are recorded.
