# Evidence

## Before

- Score: 0
- Findings: 394
- Target settings JSON parse findings: 2
- Summary: critical 0, high 237, medium 113, low 44

## After

- Score: 0
- Findings: 392
- Target settings JSON parse findings: 0
- Summary: critical 0, high 235, medium 113, low 44

## Checks

- `rtk err bun test products/eden/tests/preferences.test.ts`: passed.
- `rtk err bun run --cwd platform/desktop typecheck`: passed.
- `rtk err bun run --cwd platform/desktop build:extension eden`: passed.
- `rtk err bun run --cwd platform/desktop build:js:shell`: passed, existing Vite warnings only.
- Desloppify scan: expected exit 1 while repository findings remain; valid JSON saved to `desloppify-after.json`.
