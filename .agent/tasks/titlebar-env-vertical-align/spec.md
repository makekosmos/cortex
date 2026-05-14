# Task: Titlebar Env Vertical Align

## Goal
Align shared titlebar content vertically with the actual Window Controls Overlay region by accounting for `env(titlebar-area-y, ...)` in the Windows titlebar layout.

## Scope
- `packages/kosmos-visuals/components/Titlebar.vue`
- verification artifacts in `.agent/tasks/titlebar-env-vertical-align/`

## Acceptance Criteria
- AC1: Windows titlebar layout accounts for `titlebar-area-y` in vertical sizing/alignment.
- AC2: The fix is CSS-only and does not reintroduce overlay height synchronization logic.
- AC3: TypeScript verification for `apps/delphi/ts` passes after the change.
