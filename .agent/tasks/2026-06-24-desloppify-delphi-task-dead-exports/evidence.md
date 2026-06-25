# Evidence

## Commands

- `rg PriorityLabel|PriorityColor|ProjectStatusLabel|FrequencyLabel|SmartListTitle|SmartListIcon|SmartListColor|SmartListShortcut|SmartListTopGroup|SmartListBottomGroup products/delphi platform tests packages`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension delphi`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-delphi-task-dead-exports/desloppify-after.json`

## Results

- References check: PASS. Removed symbol names appeared only in
  `products/delphi/src/types/task.ts`.
- `typecheck`: PASS.
- `build:extension delphi`: PASS.
- Baseline scan: score 0, 523 findings; severity critical 0, high 321,
  medium 131, low 71.
- Final scan: score 0, 513 findings; severity critical 0, high 311,
  medium 131, low 71.
- `products/delphi/src/types/task.ts` `DEAD_EXPORT`: 10 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Confirmed dead constants are removed and final scoped scan is
  clean.
- AC3: PASS. Enums and exported model types remain exported; only unused
  constant maps/groups were removed.
- AC4: PASS. Relevant TypeScript checks passed.

## Deferred

- Repo-wide score remains 0 because high-volume `DEAD_FILE` and `DEAD_EXPORT`
  findings remain outside this slice.
