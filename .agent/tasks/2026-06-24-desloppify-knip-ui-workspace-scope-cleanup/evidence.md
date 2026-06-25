# Knip UI Workspace Scope Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-desktop-smoke-test-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `169`
- Severity: `HIGH 51`, `MEDIUM 76`, `LOW 42`
- `DEAD_DEPENDENCY`: `6`

## Change

- Added desktop renderer entrypoints to `knip.json` so `tailwindcss` is seen through `src/styles.css`.
- Added workspace-level `ignoreDependencies` for confirmed false positives:
  - `platform/desktop`: `@cosmos.gl/graph`, `@phosphor-icons/vue`
  - `incubator/akasha`: `@fontsource/source-serif-4`, `@vitejs/plugin-vue`
  - `incubator/arrancador`: `@tailwindcss/vite`
- Kept incubator Vite configs ignored because their nonstandard package resolution otherwise creates `UNLISTED_DEPENDENCY` noise.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-knip-ui-workspace-scope-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `163`
- Severity: `HIGH 51`, `MEDIUM 70`, `LOW 42`
- `DEAD_DEPENDENCY`: `0`

## Checks

- `rtk grep "Ð|Ñ|Рџ|�|Â|â€|Ã" knip.json -n`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-knip-ui-workspace-scope-final.json"`
