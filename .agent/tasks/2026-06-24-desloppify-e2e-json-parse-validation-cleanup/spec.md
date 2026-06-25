# Desloppify E2E JSON Parse Validation Cleanup

## Classification

FULL_LOOP. This is a focused E2E/helper validation cleanup inside the ongoing desloppify proof loop.

## Goal

Remove three `JSON_PARSE_CAST` findings in E2E tests/helpers without weakening the tests or changing headless behavior.

## Change

- Added `parseSyncVersionVector` in `extension-permissions.spec.ts` to validate parsed sync version vector entries.
- Added manifest shape validation in `extensions-contract.spec.ts` before accepting manifest JSON into the generated contract suite.
- Added Pomodoro state-file shape validation in `helpers/pomodoro-state-file.ts`, returning an explicit `PomodoroStateFileShape`.

## Verification

- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS: `rtk err powershell -NoProfile -Command "$env:KOSMOS_HEADLESS='1'; $env:KOSMOS_TEST_MODE='1'; bunx playwright test --config playwright.config.ts extension-permissions.spec.ts extensions-contract.spec.ts focus-widget-controls.spec.ts --list"`
- PASS: `rtk err powershell -NoProfile -Command "$env:KOSMOS_HEADLESS='1'; $env:KOSMOS_TEST_MODE='1'; bunx playwright test --config playwright.config.ts extension-permissions.spec.ts -g 'user-installed extension with scoped permission can write that object type'"`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-e2e-json-parse-validation-cleanup.json"`

The direct ad-hoc `tsc` command is not a valid project check for these E2E files because it lacks Playwright/project ambient types; it did expose one local parser return typing issue, which was fixed before the authoritative Playwright list check passed.
