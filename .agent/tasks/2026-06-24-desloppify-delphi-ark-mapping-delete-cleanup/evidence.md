# Delphi Ark Mapping Delete Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-dead-files-delete-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `157`
- Severity: `HIGH 46`, `MEDIUM 69`, `LOW 42`
- `DEAD_EXPORT`: `30`
- `LONG_FILE`: `21`

## Change

- Deleted unused legacy `ArkChange` mapping helpers from `products/delphi/src/services/sync/ark-types.ts`.
- Removed the now-unused legacy `Task` import from the same file.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-delphi-ark-mapping-delete-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `150`
- Severity: `HIGH 40`, `MEDIUM 68`, `LOW 42`
- `DEAD_EXPORT`: `24`
- `LONG_FILE`: `20`

## Checks

- `rtk grep "todoItemToArkChange|taskToArkChange|arkChangeToTask|arkChangeToTodoItem|projectToArkChange|arkChangeToProject|arkChangeEventType|\bArkChange\b|\bTask\b" products/delphi/src/services/sync/ark-types.ts products/delphi/src -n`
- `rtk test bun run --cwd products/delphi test:vue`
- `rtk test bun run ark:guard:writes`
- mojibake scan on `products/delphi/src/services/sync/ark-types.ts`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-delphi-ark-mapping-delete.json"`
