# Kext And Delphi Polling Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-kext-install-revert-polling-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `139`
- Severity: `HIGH 37`, `MEDIUM 60`, `LOW 42`
- `SLEEPY_TEST`: `16`

## Change

- Replaced backend warmup sleeps in `tests/e2e/delphi.spec.ts` with `waitForBackendReady`.
- Replaced Delphi Vue mount sleep with `expect.poll` against required sidebar items.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-kext-and-delphi-polling-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `137`
- Severity: `HIGH 37`, `MEDIUM 58`, `LOW 42`
- `SLEEPY_TEST`: `14`

## Checks

- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test tests/e2e/delphi.spec.ts"`
- mojibake scan on `tests/e2e/delphi.spec.ts`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-kext-and-delphi-polling.json"`
