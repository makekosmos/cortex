# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    434 |   432 |    -2 |
| Critical       |      0 |     0 |     0 |
| High           |    243 |   243 |     0 |
| Medium         |    127 |   127 |     0 |
| Low            |     64 |    62 |    -2 |

Scoped to `platform/desktop/electron/extension-installer.ts`:

- Before: 2 `UNNECESSARY_INTERMEDIATE`, plus `LARGE_FILE` and `DEAD_FILE`.
- After: only `LARGE_FILE` and `DEAD_FILE` remain.

## Change

- `installFromPath` now returns `previewDir(target)` directly.
- `listInstalledUserExtensions` now returns the merged dev/installed list directly.

Ordering and async behavior are unchanged.

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk err bun run --cwd platform/desktop build:js:shell
rtk test bun test platform/desktop/electron/no-sync-io.test.ts
rtk test bun test platform/desktop/electron/extension-installer.test.ts
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-extension-installer-intermediate-cleanup.json"
```

Results:

- Typecheck: PASS
- Shell JS build: PASS with existing Vite warnings about chunk size and `inlineDynamicImports`/`codeSplitting`
- `no-sync-io.test.ts`: PASS, 4 tests
- `extension-installer.test.ts`: PARTIAL; the installer assertion passed, but the second assertion for `extension-host.ts` still expects `if (!app.isPackaged) {`
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.

## Notes

The remaining `DEAD_FILE` finding for `extension-installer.ts` is a knip false
positive: the file is imported by `extension-host.ts`. The remaining `LARGE_FILE`
finding needs a separate split/refactor proof.
