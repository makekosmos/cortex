# 2026-06-24 Desloppify Delphi Small Helper Dead Exports

## Classification

FULL_LOOP.

## Goal

Remove a small set of confirmed unused Delphi helper exports while preserving
active helper behavior and product builds.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 504 total
- Severity: critical 0, high 303, medium 130, low 71

`desloppify` reports several single-symbol `DEAD_EXPORT` findings in Delphi.
`normalizePassphrase` is a false positive because it is imported and used by
`services/api/client.ts`, so it is intentionally out of scope.

## Scope

In scope:

- `products/delphi/src/components/utils.ts`
- `products/delphi/src/composables/useSidebarState.ts`
- `products/delphi/src/models/todoItem.ts`
- `products/delphi/src/models/index.ts`
- This task's evidence files

Out of scope:

- `products/delphi/src/helpers/normalize.ts`
- Delphi API client behavior
- Sync/schema changes
- UI behavior changes
- Broad model cleanup

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Confirmed dead helper exports are removed.**
Final scan removes the scoped `DEAD_EXPORT` findings for `absoluteUrl`,
`setSidebarHidden`, and external `createChecklistItem`, with `rg` evidence that
they had no cross-file references.

**AC3. Active helpers remain available.**
`cn`, `useSidebarState`, and checklist mutation helpers used by the app remain
exported and typechecked.

**AC4. Build/type checks pass.**
The relevant desktop/product TypeScript checks pass after the change, or any
unrelated blocker is documented with exact command output.
