# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    440 |   434 |    -6 |
| Critical       |      0 |     0 |     0 |
| High           |    243 |   243 |     0 |
| Medium         |    127 |   127 |     0 |
| Low            |     70 |    64 |    -6 |

Scoped to `platform/desktop/src/main.ts`:

- Before: 6 `UNNECESSARY_INTERMEDIATE` findings.
- After: no findings.

## Change

`rootView()` now returns `defineAsyncComponent(...)` directly for async window
roots instead of assigning a local constant and immediately returning it.

Routes and imported component paths are unchanged.

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk err bun run --cwd platform/desktop build:js:shell
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-desktop-main-async-component-cleanup.json"
```

Results:

- Typecheck: PASS
- Shell JS build: PASS with existing Vite warnings about chunk size and `inlineDynamicImports`/`codeSplitting`
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.
