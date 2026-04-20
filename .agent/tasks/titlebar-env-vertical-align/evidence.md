# Evidence: Titlebar Env Vertical Align

## Summary
- Adjusted the shared Windows titlebar layout to account for `env(titlebar-area-y, ...)`.
- This keeps titlebar content vertically aligned with the actual overlay region without reintroducing JavaScript synchronization.

## Acceptance Criteria

### AC1
Windows titlebar layout accounts for `titlebar-area-y` in vertical sizing/alignment.

Result: PASS

Evidence:
- `packages/kepler-visuals/components/Titlebar.vue` now:
  - uses `box-sizing: border-box`
  - sets Windows height/min-height to `calc(env(titlebar-area-y) + env(titlebar-area-height))`
  - sets `padding-top: env(titlebar-area-y, 0px)`
- Raw artifact: `raw/titlebar-vertical-align-diff.txt`

### AC2
The fix is CSS-only and does not reintroduce overlay height synchronization logic.

Result: PASS

Evidence:
- No renderer/main-process IPC was added.
- The change is confined to CSS in `packages/kepler-visuals/components/Titlebar.vue`.
- Raw artifact: `raw/titlebar-vertical-align-diff.txt`

### AC3
TypeScript verification for `apps/delphi/ts` passes after the change.

Result: PASS

Evidence:
- Command: `bunx tsc --noEmit`
- Result: success, exit code `0`
- Raw artifact: `raw/tsc-noemit.txt`
