# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    430 |   429 |    -1 |
| Critical       |      0 |     0 |     0 |
| High           |    243 |   243 |     0 |
| Medium         |    125 |   124 |    -1 |
| Low            |     62 |    62 |     0 |

Scoped change:

- Before: `CATCH_WRAP_NO_CAUSE` in `products/eden/src/lib/typedNotes.ts`.
- After: scoped finding removed.

## Change

`parseJsonWithContext` still throws the same contextual message, but now attaches
the caught JSON parse error as `cause`.

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk test bun test products/eden/tests/systemTypes.test.ts
rtk err bun run --cwd platform/desktop build:extension eden
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-typed-notes-error-cause.json"
```

Results:

- Typecheck: PASS
- `systemTypes.test.ts`: PASS, 2 tests / 16 expects
- Eden extension build: PASS
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.

Remaining `typedNotes` findings are schema/API cleanup candidates and need a
separate proof.
