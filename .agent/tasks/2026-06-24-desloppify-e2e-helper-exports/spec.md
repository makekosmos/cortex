# 2026-06-24 Desloppify E2E Helper Exports

## Classification

FULL_LOOP.

## Goal

Reduce real `DEAD_EXPORT` findings in e2e helper modules without changing test
behavior or weakening headless/test isolation.

## Context

The current `desloppify` scan reports 532 findings:

- Score: 0
- Severity: 0 critical, 330 high, 131 medium, 71 low
- Largest category: dead-code, mostly `DEAD_FILE` false positives for tests,
  Electron entry modules, and build entrypoints.

This slice targets the smaller, verifiable `DEAD_EXPORT` findings in helper
files where `rg` confirms no cross-file references.

## Scope

In scope:

- `tests/e2e/helpers/launch.ts`
- `tests/e2e/helpers/wait.ts`
- `tests/e2e/helpers/pomodoro-state-file.ts`
- This task's evidence files

Out of scope:

- Runtime code
- Broad `DEAD_FILE` cleanup
- Test behavior changes or coverage deletion
- Headless/window behavior

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Confirmed unused helper exports are removed.**
Final scan has fewer `DEAD_EXPORT` findings for the scoped helper files, and
each removed symbol has no cross-file references according to `rg`.

**AC3. Existing e2e helper behavior is preserved.**
Focused headless e2e checks that use these helpers still pass, or any unrelated
blocker is documented with exact command output.

**AC4. No test isolation regression.**
Changes do not remove `KOSMOS_HEADLESS`, `KOSMOS_TEST_MODE`, or isolated
`KOSMOS_DATA_DIR` behavior from `launchKepler`.
