# Settings Autorun Assertion Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-eden-block-selection-delete-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `148`
- Severity: `HIGH 39`, `MEDIUM 67`, `LOW 42`
- `WEAK_ASSERTION`: `12`

## Change

- Replaced a truthy Electron app-name assertion in `tests/e2e/settings-autorun.spec.ts` with the concrete test-slot name `Kosmos [test]`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-settings-autorun-assertion-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `147`
- Severity: `HIGH 39`, `MEDIUM 66`, `LOW 42`
- `WEAK_ASSERTION`: `11`

## Checks

- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test tests/e2e/settings-autorun.spec.ts"`
- mojibake scan on `tests/e2e/settings-autorun.spec.ts`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-settings-autorun-assertion.json"`
