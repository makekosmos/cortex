# Evidence

## Before

- Score: 9
- Findings: 381
- Target `cmGate.ts` deep-nesting findings: 1
- Summary: critical 0, high 224, medium 113, low 44

## After

- Score: 9
- Findings: 380
- Target `cmGate.ts` deep-nesting findings: 0
- Summary: critical 0, high 223, medium 113, low 44

## Checks

- Read `docs-site/apps/eden/editor.md` before editing.
- `rtk err bun test products/eden/tests/cmGate.test.ts`: passed.
- `rtk err bun run --cwd platform/desktop build:extension eden`: passed.
- `rtk err bun run --cwd platform/desktop typecheck`: passed.
- Desloppify scan: expected exit 1 while repository findings remain; valid JSON saved to `desloppify-after.json`.
