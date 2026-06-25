# 2026-06-24 Desloppify Delphi Project Model Dead Exports

## Classification

FULL_LOOP.

## Goal

Remove confirmed unused project progress helper exports from Delphi models while
preserving active project creation behavior.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 507 total
- Severity: critical 0, high 306, medium 130, low 71

`products/delphi/src/models/project.ts` has 3 `DEAD_EXPORT` findings for
`completedCount`, `totalCount`, and `progress`. `rg` shows these functions are
not referenced outside the declaring file and the barrel re-export.

## Scope

In scope:

- `products/delphi/src/models/project.ts`
- `products/delphi/src/models/index.ts`
- This task's evidence files

Out of scope:

- Delphi store behavior
- Project creation model changes
- Todo/checklist models
- Sync/schema changes
- UI changes

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Confirmed dead project helpers are removed.**
Final scan has 0 `DEAD_EXPORT` findings for `products/delphi/src/models/project.ts`,
and `rg` confirms removed symbol names had no cross-file references before
deletion.

**AC3. Active project creation remains exported.**
`createProject` and `CreateProjectParams` remain exported and existing imports
continue to typecheck.

**AC4. Build/type checks pass.**
The relevant desktop/product TypeScript checks pass after the change, or any
unrelated blocker is documented with exact command output.
