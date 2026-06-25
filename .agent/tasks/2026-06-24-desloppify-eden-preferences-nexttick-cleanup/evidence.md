# Eden Preferences NextTick Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-knip-ui-workspace-scope-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `163`
- Severity: `HIGH 51`, `MEDIUM 70`, `LOW 42`
- `SLEEPY_TEST`: `21`

## Change

- Replaced `setTimeout(0)` in `products/eden/tests/preferences.test.ts` with Vue `nextTick()`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-eden-preferences-nexttick-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `162`
- Severity: `HIGH 51`, `MEDIUM 69`, `LOW 42`
- `SLEEPY_TEST`: `20`

## Checks

- `rtk test bun run --cwd products/eden test:unit`
- mojibake scan on `products/eden/tests/preferences.test.ts`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-eden-preferences-nexttick.json"`
