# Delphi Persistence Polling Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-delphi-tasks-polling-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `135`
- Severity: `HIGH 37`, `MEDIUM 56`, `LOW 42`
- `SLEEPY_TEST`: `12`

## Change

- Replaced Delphi open/mount sleeps in `tests/e2e/delphi-persistence.spec.ts` with a visible Inbox anchor wait.
- Replaced post-reopen ARK/UI sleeps with `expect.poll` for the persisted task title.
- Replaced manual quit/timeout cleanup with `app.close()`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-delphi-persistence-polling-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `134`
- Severity: `HIGH 37`, `MEDIUM 55`, `LOW 42`
- `SLEEPY_TEST`: `11`

## Checks

- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test tests/e2e/delphi-persistence.spec.ts"`
- mojibake scan on `tests/e2e/delphi-persistence.spec.ts`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-delphi-persistence-polling.json"`
