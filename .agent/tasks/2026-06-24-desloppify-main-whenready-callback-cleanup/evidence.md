# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    421 |   420 |    -1 |
| Critical       |      0 |     0 |     0 |
| High           |    242 |   242 |     0 |
| Medium         |    124 |   123 |    -1 |
| Low            |     55 |    55 |     0 |

## Change

`app.whenReady().then(async () => { ... })` became:

```ts
void app.whenReady().then(onAppReady);

async function onAppReady(): Promise<void> {
  // existing startup body
}
```

The startup body and ordering are unchanged.

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk err bun run --cwd platform/desktop build:js:shell
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-main-whenready-callback-cleanup.json"
```

Results:

- Typecheck: PASS
- Shell JS build: PASS with existing Vite warnings
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.

Remaining `main.ts` findings are architectural: import-heavy, scattered env,
large file, and knip `DEAD_FILE` false positive.
