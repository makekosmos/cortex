# Kext Install Revert Polling Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-eden-trailing-and-kext-argv-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `142`
- Severity: `HIGH 37`, `MEDIUM 63`, `LOW 42`
- `SLEEPY_TEST`: `19`

## Change

- Replaced fixed IPC warmup sleeps in `platform/desktop/e2e/kext-install.spec.ts` with `expect.poll` for `kepler:extension:install:do`.
- Replaced fixed IPC warmup sleep in `platform/desktop/e2e/kext-revert.spec.ts` with `expect.poll` for install/list/revert handlers.
- Added explicit `KOSMOS_HEADLESS=1` and `KOSMOS_LOCK_PERMISSIONS_DISABLED=1` to the custom Electron launch envs.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-kext-install-revert-polling-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `139`
- Severity: `HIGH 37`, `MEDIUM 60`, `LOW 42`
- `SLEEPY_TEST`: `16`

## Checks

- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test --config platform/desktop/playwright.config.ts platform/desktop/e2e/kext-revert.spec.ts platform/desktop/e2e/kext-install.spec.ts"`
- mojibake scans on touched files
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-kext-install-revert-polling.json"`
