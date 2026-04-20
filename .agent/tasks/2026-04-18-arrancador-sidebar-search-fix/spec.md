# 2026-04-18 Arrancador Sidebar Search Fix

## Context

`apps/arrancador` was aligned to the Delphi TS desktop shell, but the follow-up UX is off:

- the sidebar does not expose visible top-level controls
- the quick-search trigger still sits in the titlebar instead of the sidebar

This task restores the expected chrome composition without changing unrelated app behavior.

## Acceptance Criteria

- AC1: desktop and mobile sidebar render visible top controls, including a search trigger, so the sidebar is no longer visually missing action buttons
- AC2: the titlebar no longer renders the quick-search trigger; search is initiated from the sidebar instead
- AC3: the updated shell contract is covered by tests and passes fresh `bun run typecheck` and `bun run test`
