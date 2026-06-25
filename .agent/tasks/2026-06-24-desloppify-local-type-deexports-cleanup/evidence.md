# Local Type De-exports Cleanup

## Baseline

- File: `.tmp/desloppify-after-eden-preferences-nexttick.json`
- Score: `11`
- Findings: `162`
- Severity: `HIGH 51`, `MEDIUM 69`, `LOW 42`
- `DEAD_EXPORT`: `32`

## Change

- Made `DragRect` local in `products/eden/src/composables/useBlockSelection.ts`.
- Made `ArkChange` local in `products/delphi/src/services/sync/ark-types.ts`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-local-type-deexports-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `160`
- Severity: `HIGH 49`, `MEDIUM 69`, `LOW 42`
- `DEAD_EXPORT`: `30`

## Checks

- `rtk grep "\bDragRect\b" products/eden site platform core packages -n`
- `rtk grep "\bArkChange\b" products/delphi site platform core packages -n`
- `rtk test bun run --cwd products/eden test:unit`
- `rtk test bun run --cwd products/delphi test:vue`
- `rtk test bun run ark:guard:writes`
- mojibake scan on touched files
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-local-type-deexports.json"`
