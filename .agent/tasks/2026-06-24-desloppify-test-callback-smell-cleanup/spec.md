# 2026-06-24 Desloppify Test Callback Smell Cleanup

## Classification

FULL_LOOP.

## Goal

Remove low-risk callback/test smell findings without changing runtime behavior.

## Context

The previous scan had two explicit `return undefined` findings and three
redundant `return await` findings in test/evaluate callbacks. A separate
`Promise.resolve(undefined)` finding in `CmEditor.spec.ts` was investigated but
left unchanged because alternative implementations changed test scheduling.

## Scope

In scope:

- `products/eden/tests/components/EdenSidebar.spec.ts`
- `platform/desktop/electron/raycast/view-model.ts`
- `platform/desktop/e2e/kext-install.spec.ts`
- `platform/desktop/e2e/kext-revert.spec.ts`
- This task's evidence files

Out of scope:

- E2E sleep removal
- Raycast view-model file ownership / import graph
- CmEditor stale-resolution test semantics

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Low-risk findings are removed.**
The scoped `RETURN_UNDEFINED` and `REDUNDANT_RETURN_AWAIT` findings disappear.

**AC3. Relevant checks are recorded.**
Typecheck, shell build, Raycast unit test, a focused EdenSidebar browser check,
and any relevant failed candidate checks are recorded.
