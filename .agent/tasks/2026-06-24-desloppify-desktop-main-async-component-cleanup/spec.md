# 2026-06-24 Desloppify Desktop Main Async Component Cleanup

## Classification

FULL_LOOP.

## Goal

Remove low-risk `UNNECESSARY_INTERMEDIATE` findings from the desktop renderer
bootstrap without changing hash-route behavior.

## Context

`platform/desktop/src/main.ts` uses one renderer bundle for several desktop
window roots. The `rootView()` dispatcher created async component constants and
returned them immediately.

## Scope

In scope:

- `platform/desktop/src/main.ts`
- This task's evidence files

Out of scope:

- Hash route semantics
- Window creation / Electron main process
- Visual layout or UI copy

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Route behavior is preserved.**
Each route still returns the same sync or async component as before.

**AC3. Relevant checks pass.**
Typecheck, shell JS build, and the full scan run are recorded.
