# Launcher Test Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-delphi-persistence-sleep-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `194`
- Severity: `HIGH 57`, `MEDIUM 95`, `LOW 42`
- `SLEEPY_TEST`: `25`
- `WEAK_ASSERTION`: `16`

## Change

- Replaced the truthy app-name assertion with the concrete test-slot app name.
- Replaced the window startup sleep with `app.firstWindow()` plus `domcontentloaded`.
- Changed the second test slug to avoid a local stale ACL test-data directory (`tests/.e2e/launcher-windows`).

## Result

- File: `.agent/tasks/2026-06-24-desloppify-launcher-test-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `192`
- Severity: `HIGH 57`, `MEDIUM 93`, `LOW 42`
- `SLEEPY_TEST`: `24`
- `WEAK_ASSERTION`: `15`

## Checks

- `rtk grep "Ð|Ñ|Рџ|�|Â|â€|Ã|toBeTruthy|setTimeout\(r|new Promise\(\(r\) => setTimeout" tests/e2e/launcher.spec.ts`
- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test --config playwright.config.ts tests/e2e/launcher.spec.ts"`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-launcher-test-cleanup.json"`
