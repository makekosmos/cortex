# Evidence

## Before

- Score: 9
- Findings: 386
- Target `extensions-contract.spec.ts` deep-nesting findings: 4
- Summary: critical 0, high 229, medium 113, low 44

## After

- Score: 9
- Findings: 382
- Target `extensions-contract.spec.ts` deep-nesting findings: 0
- Summary: critical 0, high 225, medium 113, low 44

## Checks

- Root Playwright `extensions-contract.spec.ts --list` with headless/test env: passed.
- Root Playwright discovery test with headless/test env: passed.
- `rtk err bun run --cwd platform/desktop typecheck`: passed.
- Desloppify scan: expected exit 1 while repository findings remain; valid JSON saved to `desloppify-after.json`.
