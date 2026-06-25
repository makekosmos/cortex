# Eden Vitest Browser Dependency Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-eden-tailwind-manifest-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `177`
- Severity: `HIGH 51`, `MEDIUM 84`, `LOW 42`
- `DEAD_DEPENDENCY`: `8`

## Change

- Removed redundant direct Eden dev dependency `@vitest/browser`.
- Kept `@vitest/browser-playwright`, which is the package imported by the browser runner and brings browser support transitively.
- Ran `bun install` to refresh `bun.lock`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-eden-vitest-browser-dependency-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `176`
- Severity: `HIGH 51`, `MEDIUM 83`, `LOW 42`
- `DEAD_DEPENDENCY`: `7`

## Checks

- `rtk bun install`
- `rtk test bun run --cwd products/eden test:unit`
- `rtk test bun run --cwd products/eden test:vue`
  - The browser runner starts, but the suite fails on existing browser/preload/content assertions, including `window.kepler.ark недоступен`.
- `rtk grep "@vitest/browser\"|Ð|Ñ|Рџ|�|Â|â€|Ã" products/eden/package.json bun.lock`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-eden-vitest-browser-dep.json"`
