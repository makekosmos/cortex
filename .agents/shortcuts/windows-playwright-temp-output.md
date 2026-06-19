# Playwright Temp Output on Windows

## Trigger

Use when a focused Playwright run fails on Windows with `EPERM` around
`test-results/.last-run.json`, output-dir creation, or worker spawn.

## Symptom

The test run starts, then fails before or after the first test with file-lock or
permission errors unrelated to the app logic.

## Do This

Run the slice with a writable temp output dir and a simple reporter, then
rebuild the affected binary if the test is exercising fresh runtime code:

```powershell
rtk bunx playwright test --config platform/desktop/playwright.config.ts --reporter=line --output C:\Users\kirill\AppData\Local\Temp\playwright-out <spec-or-grep>
rtk cargo build -p kepler-backend
```

If Cargo hits `target\debug\.cargo-lock` permission errors, rerun the build
with escalated permissions.

## Avoid

Do not spend time on the reporter error itself if the app logic never ran.
Avoid leaving the run pointed at a locked repo-local `test-results` folder when
you only need a focused verification pass.

## Promote To Skill When

When Windows Playwright or Cargo workspace locking causes this same class of
failure across multiple tasks.
