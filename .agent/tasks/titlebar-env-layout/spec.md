# Task: Titlebar Env Layout

## Goal
Use `env(titlebar-area-*)` CSS environment variables for the Delphi TS shared titlebar layout on Windows instead of heuristic safe-area padding.

## Scope
- `packages/kosmos-visuals/components/Titlebar.vue`
- verification artifacts in `.agent/tasks/titlebar-env-layout/`

## Acceptance Criteria
- AC1: Windows titlebar height uses `env(titlebar-area-height, ...)`.
- AC2: Windows titlebar horizontal safe-area no longer relies on percentage padding and instead derives from `titlebar-area-x` and `titlebar-area-width`.
- AC3: TypeScript verification for `apps/delphi/ts` passes after the change.
