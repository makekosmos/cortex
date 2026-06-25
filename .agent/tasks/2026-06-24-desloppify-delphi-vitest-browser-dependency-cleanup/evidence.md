# Delphi Vitest Browser Dependency Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-eden-vitest-browser-dependency-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `176`
- Severity: `HIGH 51`, `MEDIUM 83`, `LOW 42`
- `DEAD_DEPENDENCY`: `7`

## Change

- Removed redundant direct Delphi dev dependency `@vitest/browser`.
- Kept `@vitest/browser-playwright`, which is the package imported by `vitest.config.ts`.
- Ran `bun install` to refresh `bun.lock`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-delphi-vitest-browser-dependency-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `175`
- Severity: `HIGH 51`, `MEDIUM 82`, `LOW 42`
- `DEAD_DEPENDENCY`: `6`

## Checks

- `rtk bun install`
- `rtk test bun run --cwd products/delphi test:vue`
- `rtk grep "@vitest/browser\"|Ð|Ñ|Рџ|�|Â|â€|Ã" products/delphi/package.json bun.lock`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-delphi-vitest-browser-dep.json"`
