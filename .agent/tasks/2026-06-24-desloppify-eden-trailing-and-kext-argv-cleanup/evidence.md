# Eden Trailing Assertion And Kext Argv Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-safe-deexports-and-contract-assertions-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `144`
- Severity: `HIGH 37`, `MEDIUM 65`, `LOW 42`
- `SLEEPY_TEST`: `20`
- `WEAK_ASSERTION`: `10`

## Change

- Replaced the fallback DOM truthy assertion in `tests/e2e/eden-trailing-paragraph.spec.ts` with a concrete `toMatchObject` structure check.
- Replaced the fixed startup sleep in `platform/desktop/e2e/kext-argv.spec.ts` with `expect.poll`.
- Added explicit `KOSMOS_HEADLESS=1` and `KOSMOS_LOCK_PERMISSIONS_DISABLED=1` to the custom Electron launch env in `kext-argv`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-eden-trailing-and-kext-argv-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `142`
- Severity: `HIGH 37`, `MEDIUM 63`, `LOW 42`
- `SLEEPY_TEST`: `19`
- `WEAK_ASSERTION`: `9`

## Checks

- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test --config platform/desktop/playwright.config.ts platform/desktop/e2e/kext-argv.spec.ts"`
- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test tests/e2e/eden-trailing-paragraph.spec.ts"` (failed on existing Eden mount/state assertions, not the changed fallback assertion)
- mojibake scans on touched files
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-eden-trailing-and-kext-argv.json"`
