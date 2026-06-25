# Desktop Smoke Test Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-extension-api-compat-test-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `172`
- Severity: `HIGH 51`, `MEDIUM 79`, `LOW 42`
- `SLEEPY_TEST`: `22`
- `WEAK_ASSERTION`: `14`

## Change

- Added modern test isolation env to the custom Electron launcher:
  - `KOSMOS_DATA_DIR`
  - `KOSMOS_TEST_MODE=1`
  - `KOSMOS_HEADLESS=1`
  - `KOSMOS_LOCK_PERMISSIONS_DISABLED=1`
- Replaced truthy app path/name assertions with concrete app path and test-slot name checks.
- Removed arbitrary startup sleep.
- Removed `expect(true).toBe(true)` from the close smoke.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-desktop-smoke-test-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `169`
- Severity: `HIGH 51`, `MEDIUM 76`, `LOW 42`
- `SLEEPY_TEST`: `21`
- `WEAK_ASSERTION`: `12`

## Checks

- `rtk grep "Ð|Ñ|Рџ|�|Â|â€|Ã|toBeTruthy|setTimeout\(r|new Promise\(\(r\) => setTimeout|expect\(true\)" platform/desktop/e2e/smoke.spec.ts`
- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test --config playwright.config.ts e2e/smoke.spec.ts"` from `platform/desktop`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-desktop-smoke-cleanup.json"`
