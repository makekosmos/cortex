# Extension API Compat Test Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-delphi-vitest-browser-dependency-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `175`
- Severity: `HIGH 51`, `MEDIUM 82`, `LOW 42`
- `SLEEPY_TEST`: `24`
- `WEAK_ASSERTION`: `15`

## Change

- Removed the stale app-name smoke test from `extension-api-compat.spec.ts`; it did not test semver compatibility despite its title/comments.
- Added explicit `KOSMOS_HEADLESS=1` to custom Electron launch env.
- Replaced IPC-handler warmup sleep with `expect.poll`.
- Replaced post-open sleep with `app.waitForEvent("window")`.
- Replaced boolean title scan with polling on the actual incompat window title.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-extension-api-compat-test-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `172`
- Severity: `HIGH 51`, `MEDIUM 79`, `LOW 42`
- `SLEEPY_TEST`: `22`
- `WEAK_ASSERTION`: `14`

## Checks

- `rtk grep "Ð|Ñ|Рџ|�|Â|â€|Ã|toBeTruthy|setTimeout\(r|new Promise\(\(r\) => setTimeout|waitForTimeout|satisfiesSemver" platform/desktop/e2e/extension-api-compat.spec.ts`
- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test --config playwright.config.ts e2e/extension-api-compat.spec.ts"` from `platform/desktop`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-extension-api-compat-cleanup.json"`
