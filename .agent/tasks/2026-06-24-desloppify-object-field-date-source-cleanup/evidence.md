# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    415 |   414 |    -1 |
| Critical       |      0 |     0 |     0 |
| High           |    242 |   242 |     0 |
| Medium         |    123 |   122 |    -1 |
| Low            |     50 |    50 |     0 |

## Change

`formatReadableRussianDate` now uses an explicit `if`/`else if` chain to select
the date source instead of a nested ternary. Semantics are unchanged:

- `number` -> use the number
- `string` -> `Date.parse(value)`
- anything else -> `NaN`

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk err bun run --cwd platform/desktop build:extension eden
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-object-field-date-source-cleanup.json"
```

Results:

- Typecheck: PASS
- Eden extension build: PASS
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.
