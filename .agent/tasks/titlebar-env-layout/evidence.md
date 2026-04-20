# Evidence: Titlebar Env Layout

## Summary
- Switched the shared Windows titlebar layout to CSS `env(titlebar-area-*)`.
- Removed heuristic percentage-based right safe-area from the Windows titlebar layout.
- Kept the solution CSS-only, with no renderer/main-process synchronization.

## Acceptance Criteria

### AC1
Windows titlebar height uses `env(titlebar-area-height, ...)`.

Result: PASS

Evidence:
- `packages/kepler-visuals/components/Titlebar.vue` now sets:
  - `height: env(titlebar-area-height, var(--kepler-titlebar-height))`
  - `min-height: env(titlebar-area-height, var(--kepler-titlebar-height))`
- Raw artifact: `raw/titlebar-env-diff.txt`

### AC2
Windows titlebar horizontal safe-area no longer relies on percentage padding and instead derives from `titlebar-area-x` and `titlebar-area-width`.

Result: PASS

Evidence:
- `packages/kepler-visuals/components/Titlebar.vue` now computes:
  - left safe area from `env(titlebar-area-x, 0px)`
  - right safe area from `100vw - env(titlebar-area-x) - env(titlebar-area-width)`
- The previous percentage fallback padding was removed.
- Raw artifact: `raw/titlebar-env-diff.txt`

### AC3
TypeScript verification for `apps/delphi/ts` passes after the change.

Result: PASS

Evidence:
- Command: `bunx tsc --noEmit`
- Result: success, exit code `0`
- Raw artifact: `raw/tsc-noemit.txt`

## Notes
- This change aligns the shared titlebar layout with Window Controls Overlay environment variables instead of hand-tuned geometry.
- Runtime visual verification still depends on a local Electron window, so any remaining alignment issue should be tuned by adjusting the safe-area formula rather than reintroducing custom overlay height logic.
