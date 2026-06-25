# Root Dependency Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-docs-site-dependency-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `181`
- Severity: `HIGH 51`, `MEDIUM 88`, `LOW 42`
- `DEAD_DEPENDENCY`: `10`

## Change

- Removed unused root dev dependencies:
  - `electron-playwright-helpers`
  - `vue-router`
- Kept package-level `vue-router` declarations where source imports it directly.
- Ran `bun install` to refresh `bun.lock`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-root-dependency-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `179`
- Severity: `HIGH 51`, `MEDIUM 86`, `LOW 42`
- `DEAD_DEPENDENCY`: `8`

## Checks

- `rtk proxy rg -n "electron-playwright-helpers" . --glob '!node_modules/**' --glob '!docs-site/node_modules/**' --glob '!products/**/dist/**' --glob '!platform/desktop/dist*/**' --glob '!platform/desktop/release/**'`
- `rtk proxy rg -n "vue-router" . --glob '!node_modules/**' --glob '!docs-site/node_modules/**' --glob '!products/**/dist/**' --glob '!platform/desktop/dist*/**' --glob '!platform/desktop/release/**' --glob '!docs-site/public/full-llms.txt'`
- `rtk bun install`
- `rtk grep "electron-playwright-helpers|Ð|Ñ|Рџ|�|Â|â€|Ã" package.json bun.lock`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-root-deps.json"`
