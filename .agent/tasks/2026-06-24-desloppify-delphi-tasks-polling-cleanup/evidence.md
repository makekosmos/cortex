# Delphi Tasks Polling Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-kext-and-delphi-polling-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `137`
- Severity: `HIGH 37`, `MEDIUM 58`, `LOW 42`
- `SLEEPY_TEST`: `14`

## Change

- Replaced Delphi task seeding warmup sleep with `waitForBackendReady`.
- Replaced Delphi ARK/UI load sleeps with `expect.poll` for the seeded task title.
- Replaced manual process-exit timeout cleanup with `app.close()`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-delphi-tasks-polling-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `135`
- Severity: `HIGH 37`, `MEDIUM 56`, `LOW 42`
- `SLEEPY_TEST`: `12`

## Checks

- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test tests/e2e/delphi-tasks.spec.ts"`
- mojibake scan on `tests/e2e/delphi-tasks.spec.ts`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-delphi-tasks-polling.json"`
