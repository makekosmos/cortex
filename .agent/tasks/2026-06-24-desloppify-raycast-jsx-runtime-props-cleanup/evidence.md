# Evidence

## Before

- Score: 0
- Findings: 407
- Target findings in `packages/raycast-api/src/jsx-runtime.ts`: 2
- Summary: critical 0, high 242, medium 117, low 48

## After

- Score: 0
- Findings: 405
- Target findings in `packages/raycast-api/src/jsx-runtime.ts`: 0
- Summary: critical 0, high 242, medium 117, low 46

## Checks

- `rtk err bun test tests/unit/raycast-command-runner.test.ts`: passed.
- `rtk err bun run --cwd platform/desktop typecheck`: passed.
- Desloppify scan: expected exit 1 while repository findings remain; valid JSON saved to `desloppify-after.json`.
