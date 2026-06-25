# 2026-06-24 Desloppify Eden Helper Dead Exports

## Classification

FULL_LOOP.

## Goal

Remove confirmed unused exports from Eden e2e helper modules without changing
active test flows or headless/test isolation behavior.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 513 total
- Severity: critical 0, high 311, medium 131, low 71

`tests/e2e/helpers/eden.ts` and `tests/e2e/helpers/eden-doc.ts` still have
`DEAD_EXPORT` findings. `rg` shows the target symbols are either local-only or
unused.

## Scope

In scope:

- `tests/e2e/helpers/eden.ts`
- `tests/e2e/helpers/eden-doc.ts`
- This task's evidence files

Out of scope:

- Eden runtime/product code
- Eden UI behavior
- Broad e2e rewrites
- Headless/window behavior

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Confirmed dead helper exports are removed.**
Final scan has 0 `DEAD_EXPORT` findings for the scoped helper files, and `rg`
confirms removed symbol names had no cross-file references before deletion.

**AC3. Existing Eden e2e helper behavior is preserved.**
Focused Eden headless e2e checks that use the remaining helpers pass, or any
unrelated blocker is documented with exact command output.

**AC4. No headless/test isolation regression.**
Changes do not alter `launchKepler`, `KOSMOS_HEADLESS`, `KOSMOS_TEST_MODE`, or
isolated `KOSMOS_DATA_DIR` behavior.
