# Eden Tailwind Manifest Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-root-dependency-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `179`
- Severity: `HIGH 51`, `MEDIUM 86`, `LOW 42`
- `UNLISTED_DEPENDENCY`: `2`

## Change

- Added Eden dev dependencies that are directly used by its local dev Vite/CSS entrypoints:
  - `@tailwindcss/vite`
  - `tailwindcss`
- Ran `bun install` to refresh `bun.lock`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-eden-tailwind-manifest-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `177`
- Severity: `HIGH 51`, `MEDIUM 84`, `LOW 42`
- `UNLISTED_DEPENDENCY`: `0`

## Checks

- `rtk bun install`
- `rtk test bun run --cwd products/eden test:unit`
- `rtk grep "Ð|Ñ|Рџ|�|Â|â€|Ã" products/eden/package.json bun.lock`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-eden-tailwind-manifest.json"`
