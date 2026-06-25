# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    444 |   442 |    -2 |
| Critical       |      0 |     0 |     0 |
| High           |    245 |   245 |     0 |
| Medium         |    128 |   127 |    -1 |
| Low            |     71 |    70 |    -1 |

Scoped to `products/eden/src/lib/markdownFrontmatter.ts`:

- Before: `RETURN_UNDEFINED` and `CATCH_WRAP_NO_CAUSE`.
- After: no findings.

## Change

- `parseEntryHeaderProps` now preserves the original JSON parse failure via `new Error(message, { cause: error })`.
- `normalizeFrontmatterValue` now uses `return;` for the `undefined` branch.

The invalid JSON error message and serializer semantics are unchanged.

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk test bun test products/eden/tests/obsidianVault.test.ts
rtk err bun run --cwd platform/desktop build:extension eden
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-markdown-frontmatter-active-cleanup.json"
```

Results:

- Typecheck: PASS
- `obsidianVault.test.ts`: PASS, 12 tests / 51 expects
- Eden extension build: PASS
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.
