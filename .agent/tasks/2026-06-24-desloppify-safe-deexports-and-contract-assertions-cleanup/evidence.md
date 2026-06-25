# Safe De-exports And Contract Assertions Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-settings-autorun-assertion-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `147`
- Severity: `HIGH 39`, `MEDIUM 66`, `LOW 42`
- `DEAD_EXPORT`: `23`
- `WEAK_ASSERTION`: `11`

## Change

- Replaced the extension contract object truthy assertion with a concrete `toMatchObject` contract.
- Deleted unused legacy `Task` type from `products/delphi/src/types/task.ts`.
- De-exported internal-only `ArkObjectSummaryRecord` in `core/ark/packages/ark/src/ark-client.ts`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-safe-deexports-and-contract-assertions-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `144`
- Severity: `HIGH 37`, `MEDIUM 65`, `LOW 42`
- `DEAD_EXPORT`: `21`
- `WEAK_ASSERTION`: `10`

## Checks

- `rtk test cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test tests/e2e/settings-autorun.spec.ts"`
- `rtk proxy cmd /c "set KOSMOS_HEADLESS=1&& bunx playwright test tests/e2e/extensions-contract.spec.ts"` (4 passed, `eden` blocked before test logic by stale ACL `EPERM` on `tests/.e2e/contract-eden`)
- `rtk test bun run --cwd products/delphi test:vue`
- `rtk test bun run --cwd core/ark/packages/ark test`
- `rtk test bun run ark:guard:writes`
- `rtk test bun run --cwd core/ark/packages/ark typecheck` (blocked by missing package-local `typescript`)
- `rtk test bunx tsc -p core/ark/packages/ark/tsconfig.json --noEmit` (blocked by missing `@types/node`)
- mojibake scans on touched files
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-safe-deexports-and-contract-assertions.json"`
