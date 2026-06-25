# 2026-06-24 Desloppify E2E Electron Import Style Cleanup

## Classification

FULL_LOOP.

## Goal

Remove remaining Electron binary `MIXED_IMPORT_STYLE` findings from Playwright
e2e helpers/specs.

## Context

Several ESM e2e files used `createRequire` only to call `require("electron")`.
The default ESM import from `electron` resolves to the same executable path in
this repo.

## Scope

In scope:

- `tests/e2e/helpers/launch.ts`
- `platform/desktop/e2e/dictation.spec.ts`
- `platform/desktop/e2e/extension-api-compat.spec.ts`
- `platform/desktop/e2e/kext-argv.spec.ts`
- `platform/desktop/e2e/smoke.spec.ts`
- This task's evidence files

Out of scope:

- Sleepy e2e waits
- Weak assertion improvements
- Full e2e execution

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Electron launch path is preserved.**
Each changed file still passes the Electron binary path as `executablePath`.

**AC3. Relevant checks are recorded.**
Playwright listing/parsing, shell build, and the full scan run are recorded.
