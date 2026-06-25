# Desloppify Kepler Shim Object Type Style Cleanup

## Classification

FULL_LOOP. This is a narrow Eden shim cleanup inside the ongoing desloppify proof loop.

## Goal

Remove a scanner-reported nested ternary in `products/eden/src/lib/kepler-api-shim.ts` without changing object type presentation defaults or ARK behavior.

## Change

- Extracted built-in object type icon selection to `iconForObjectTypeId`.
- Extracted built-in object type color selection to `colorForObjectTypeId`.
- Kept the exact default mappings:
  - `game_obj` -> `game-controller`, `#ef4444`
  - `task_obj` -> `checkmark-circle`, `#f59e0b`
  - fallback -> `document-text`, `#2aa7ee`

## Verification

- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS: `rtk err bun run --cwd platform/desktop build:extension eden`
- PASS: `rtk err bunx vitest run tests/components/keplerApiShim.spec.ts --browser=chromium`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-kepler-shim-object-type-style-cleanup.json"`

The desloppify scan exits 1 because findings remain, but it produced valid JSON and removed the targeted `NESTED_TERNARY` finding.
