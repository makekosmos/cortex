# Eden Dead Components And Exports Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-launcher-test-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `192`
- Severity: `HIGH 57`, `MEDIUM 93`, `LOW 42`
- `DEAD_FILE`: `6`
- `DEAD_EXPORT`: `35`

## Change

- Deleted unused Eden files:
  - `products/eden/src/WikilinkList.vue`
  - `products/eden/src/components/BlockSelectionOverlay.vue`
  - `products/eden/src/components/TaskStatusIcon.vue`
- Removed unused `sortEntries()` from `products/eden/src/components/sidebar/types.ts`.
- Made workout/exercise system type IDs local constants in `products/eden/src/lib/systemTypes.ts`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-eden-dead-components-exports-cleanup-2/desloppify-after.json`
- Score: `10`
- Findings: `186`
- Severity: `HIGH 51`, `MEDIUM 93`, `LOW 42`
- `DEAD_FILE`: `3`
- `DEAD_EXPORT`: `32`

## Checks

- `rtk grep "WikilinkList|BlockSelectionOverlay|TaskStatusIcon|sortEntries|SYSTEM_TYPE_WORKOUT_ID|SYSTEM_TYPE_EXERCISE_ID" products/eden/src`
- `rtk grep "Ð|Ñ|Рџ|�|Â|â€|Ã" products/eden/src/components/sidebar/types.ts products/eden/src/lib/systemTypes.ts`
- `rtk test bun run --cwd products/eden test:unit`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-eden-dead-components-exports.json"`
