# Evidence

## Before

- Score: 0
- Findings: 391
- Target Akasha JSON parse findings: 2
- Summary: critical 0, high 234, medium 113, low 44

## After

- Score: 0
- Findings: 389
- Target Akasha JSON parse findings: 0
- Summary: critical 0, high 232, medium 113, low 44

## Checks

- `rtk err bun run --cwd platform/desktop build:extension akasha`: passed.
- `rtk err bun run --cwd platform/desktop typecheck`: passed.
- Desloppify scan: expected exit 1 while repository findings remain; valid JSON saved to `desloppify-after.json`.
