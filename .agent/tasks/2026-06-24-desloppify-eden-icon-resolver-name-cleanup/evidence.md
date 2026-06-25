# Evidence

## Before

- Score: 0
- Findings: 405
- Target findings in `products/eden/src/lib/iconResolver.ts`: 1
- Summary: critical 0, high 242, medium 117, low 46

## After

- Score: 0
- Findings: 404
- Target findings in `products/eden/src/lib/iconResolver.ts`: 0
- Summary: critical 0, high 242, medium 116, low 46

## Checks

- `rtk err bun test products/eden/tests/systemTypes.test.ts`: passed.
- `rtk err bun run --cwd platform/desktop typecheck`: passed.
- `rtk err bun run --cwd platform/desktop build:extension eden`: passed.
- Desloppify scan: expected exit 1 while repository findings remain; valid JSON saved to `desloppify-after.json`.
