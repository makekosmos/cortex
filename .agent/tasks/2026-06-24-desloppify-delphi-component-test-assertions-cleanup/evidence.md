# Delphi Component Test Assertions Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-delphi-structured-clone-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `200`
- Severity: `HIGH 57`, `MEDIUM 100`, `LOW 43`
- `WEAK_ASSERTION`: `19`

## Change

- Updated `InfoCard.spec.ts` to assert the real root class contract.
- Updated `Skeleton.spec.ts` to use explicit root guards and `classList.contains(...)`.
- Removed stale assertions for old Tailwind rounded classes.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-delphi-component-test-assertions-cleanup/desloppify-after.json`
- Score: `10`
- Findings: `198`
- Severity: `HIGH 57`, `MEDIUM 98`, `LOW 43`
- `WEAK_ASSERTION`: `17`

## Checks

- `rtk test bun run --cwd products/delphi test:vue`
  - Passed.
  - Vite still prints the existing `decodeEntities option is passed but will be ignored in non-browser builds` warning.
- `rtk grep "Ð|Ñ|Рџ|�|Â|â€|Ã|not\.toBeNull|toMatch\(/rounded" products/delphi/tests/components/InfoCard.spec.ts products/delphi/tests/components/Skeleton.spec.ts`
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-delphi-component-tests.json"`
