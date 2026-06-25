# 2026-06-24 Desloppify Kext E2E Import Style Cleanup

## Classification

FULL_LOOP.

## Goal

Remove local `MIXED_IMPORT_STYLE` findings from kext Playwright e2e specs
without changing Electron executable resolution.

## Context

The kext install/revert specs are ESM files but used `createRequire` only to
load the Electron binary path. A default ESM import from `electron` resolves to
the same binary path in this repository.

## Scope

In scope:

- `platform/desktop/e2e/kext-install.spec.ts`
- `platform/desktop/e2e/kext-revert.spec.ts`
- This task's evidence files

Out of scope:

- Fixed sleep replacement
- Full Electron e2e install/revert execution
- Other e2e files that still use `createRequire`

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Electron binary resolution is preserved.**
The specs still pass the Electron package path as `executablePath`.

**AC3. Relevant checks are recorded.**
Playwright listing/parsing, shell build, and the full scan run are recorded.
