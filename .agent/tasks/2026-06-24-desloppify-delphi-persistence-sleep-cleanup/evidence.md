# Delphi Persistence Sleep Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-commands-architecture-assertions-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `196`
- Severity: `HIGH 57`, `MEDIUM 97`, `LOW 42`
- `SLEEPY_TEST`: `27`

## Change

- Removed the redundant pre-open sleep before `openDelphi()`.
- Replaced the post-`window.close()` sleep with `delphi.waitForEvent("close")`.
- Kept the process-exit timeout guard in `finally`; that finding is a guard false positive, not an app-state wait.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-delphi-persistence-sleep-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `194`
- Severity: `HIGH 57`, `MEDIUM 95`, `LOW 42`
- `SLEEPY_TEST`: `25`

## Checks

- `rtk grep "Ð|Ñ|Рџ|�|Â|â€|Ã|setTimeout\(r|new Promise\(\(r\) => setTimeout" tests/e2e/delphi-persistence.spec.ts`
- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test --config playwright.config.ts tests/e2e/delphi-persistence.spec.ts"`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-delphi-persistence-sleeps.json"`
