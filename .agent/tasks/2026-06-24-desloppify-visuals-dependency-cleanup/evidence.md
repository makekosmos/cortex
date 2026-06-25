# Visuals Dependency Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-eden-dead-components-exports-cleanup-2/desloppify-after.json`
- Score: `10`
- Findings: `186`
- Severity: `HIGH 51`, `MEDIUM 93`, `LOW 42`
- `DEAD_DEPENDENCY`: `15`

## Change

- Removed redundant direct dev dependencies from `packages/visuals/package.json`:
  - `@storybook/vue3`
  - `vue-component-type-helpers`
- Kept `@storybook/vue3-vite`, which is the package imported by stories and already brings those packages transitively.
- Ran `bun install` to refresh `bun.lock`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-visuals-dependency-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `184`
- Severity: `HIGH 51`, `MEDIUM 91`, `LOW 42`
- `DEAD_DEPENDENCY`: `13`

## Checks

- `rtk bun install`
- `rtk test bun run --cwd packages/visuals test`
- `rtk grep "Ð|Ñ|Рџ|�|Â|â€|Ã" packages/visuals/package.json bun.lock`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-visuals-deps.json"`
