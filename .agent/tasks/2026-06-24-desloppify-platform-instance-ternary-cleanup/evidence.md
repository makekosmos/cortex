# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    414 |   410 |    -4 |
| Critical       |      0 |     0 |     0 |
| High           |    242 |   242 |     0 |
| Medium         |    122 |   118 |    -4 |
| Low            |     50 |    50 |     0 |

## Change

Replaced nested ternaries with explicit helper functions:

- `platformMarker()` in both preload files.
- `instanceKindForSlot()`, `productNameForSlot()`, and `hotkeyForSlot()` in
  `instance.ts`.

The value mappings are unchanged.

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk err bun run --cwd platform/desktop build:js:shell
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-platform-instance-ternary-cleanup.json"
```

Results:

- Typecheck: PASS
- Shell JS build: PASS with existing Vite warnings
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.

Remaining findings in these files are architectural or entrypoint false
positives: preload long file, instance scattered env, and knip `DEAD_FILE`.
