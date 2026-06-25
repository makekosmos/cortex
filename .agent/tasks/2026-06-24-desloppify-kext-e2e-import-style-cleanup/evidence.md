# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    423 |   421 |    -2 |
| Critical       |      0 |     0 |     0 |
| High           |    242 |   242 |     0 |
| Medium         |    124 |   124 |     0 |
| Low            |     57 |    55 |    -2 |

## Change

`kext-install.spec.ts` and `kext-revert.spec.ts` now import the Electron binary
path with:

```ts
import electronBinary from "electron";
```

This replaces `createRequire(import.meta.url)` plus `require("electron")`.

## Verification

```powershell
$env:KOSMOS_HEADLESS='1'; rtk err bunx playwright test --config platform/desktop/playwright.config.ts platform/desktop/e2e/kext-install.spec.ts platform/desktop/e2e/kext-revert.spec.ts --list
rtk err bun run --cwd platform/desktop build:js:shell
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-kext-e2e-import-style-cleanup.json"
```

Results:

- Playwright listing/parsing: PASS
- Shell JS build: PASS with existing Vite warnings
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.

Remaining `SLEEPY_TEST` findings in these specs need a separate e2e readiness
proof.
