# Eden Block Selection Delete Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-delphi-ark-mapping-delete-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `150`
- Severity: `HIGH 40`, `MEDIUM 68`, `LOW 42`
- `DEAD_EXPORT`: `24`
- `LONG_FILE`: `20`

## Change

- Deleted the unused `useBlockSelection()` composable body from `products/eden/src/composables/useBlockSelection.ts`.
- Kept the tested `computeBlockSelectionAutoScrollDelta()` helper.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-eden-block-selection-delete-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `148`
- Severity: `HIGH 39`, `MEDIUM 67`, `LOW 42`
- `DEAD_EXPORT`: `23`
- `LONG_FILE`: `19`

## Checks

- `rtk grep "useBlockSelection|computeBlockSelectionAutoScrollDelta" products/eden products/delphi site platform core packages -n`
- `rtk test bun run --cwd products/eden test:unit`
- mojibake scan on `products/eden/src/composables/useBlockSelection.ts`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-eden-block-selection-delete.json"`
