# Evidence

## Before

- Score: 9
- Findings: 382
- Target `extension-ark-bridge.spec.ts` deep-nesting findings: 1
- Summary: critical 0, high 225, medium 113, low 44

## After

- Score: 9
- Findings: 381
- Target `extension-ark-bridge.spec.ts` deep-nesting findings: 0
- Summary: critical 0, high 224, medium 113, low 44

## Checks

- Root Playwright `extension-ark-bridge.spec.ts --list` with headless/test env: passed.
- `rtk err bun run --cwd platform/desktop typecheck`: passed.
- Desloppify scan: expected exit 1 while repository findings remain; valid JSON saved to `desloppify-after.json`.
