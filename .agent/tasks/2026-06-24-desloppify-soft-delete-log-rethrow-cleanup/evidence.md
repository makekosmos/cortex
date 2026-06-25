# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    424 |   423 |    -1 |
| Critical       |      0 |     0 |     0 |
| High           |    243 |   242 |    -1 |
| Medium         |    124 |   124 |     0 |
| Low            |     57 |    57 |     0 |

## Change

Removed the no-op catch/log/rethrow wrapper from
`products/eden/src/lib/kepler-api-shim.ts::softDeleteTask`.

The function still:

- Reads the task through `get_object`.
- Returns for missing or already-deleted objects.
- Writes `updatedAt` and `deletedAt` through `upsert_object`.
- Propagates errors to the caller.

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk err bun run --cwd platform/desktop build:extension eden
rtk err bunx vitest run tests/components/keplerApiShim.spec.ts --browser=chromium
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-soft-delete-log-rethrow-cleanup.json"
```

Results:

- Typecheck: PASS
- Eden extension build: PASS
- `keplerApiShim.spec.ts`: PASS, 11 tests
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.

## Notes

The remaining task object registration `LOG_AND_RETHROW` findings were left in
place because their catches reset cached retry state before rethrowing.
