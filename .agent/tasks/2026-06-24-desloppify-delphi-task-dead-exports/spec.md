# 2026-06-24 Desloppify Delphi Task Dead Exports

## Classification

FULL_LOOP.

## Goal

Remove confirmed unused exported constants from Delphi task type definitions to
reduce real `DEAD_EXPORT` findings without changing persisted models or UI
behavior.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 523 total
- Severity: critical 0, high 321, medium 131, low 71

`products/delphi/src/types/task.ts` has 10 `DEAD_EXPORT` findings for label,
color, shortcut, and smart-list grouping maps. `rg` shows no references outside
the declaring file.

## Scope

In scope:

- `products/delphi/src/types/task.ts`
- This task's evidence files

Out of scope:

- Delphi data model type changes
- Enum value changes
- UI copy/layout changes
- Sync/schema changes
- Broad product cleanup outside the confirmed dead constants

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Confirmed dead constants are removed.**
Final scan has 0 `DEAD_EXPORT` findings for `products/delphi/src/types/task.ts`,
and `rg` confirms removed symbol names had no cross-file references before
deletion.

**AC3. Delphi models remain compatible.**
Enums and exported data model types in `task.ts` remain exported; only unused
constant maps/groups are removed.

**AC4. Build/type checks pass.**
The relevant desktop/product TypeScript build check passes after the change, or
any unrelated blocker is documented with exact command output.
