# Evidence

## Before

- Score: 0
- Findings: 399
- Target Eden JSON parse findings: 2
- Summary: critical 0, high 242, medium 113, low 44

## After

- Score: 0
- Findings: 397
- Target Eden JSON parse findings: 0
- Summary: critical 0, high 240, medium 113, low 44

## Checks

- `rtk err bun test products/eden/tests/content.test.ts`: passed.
- `rtk err bunx vitest run tests/components/CmConvert.spec.ts tests/components/EntryTitle.spec.ts --browser=chromium`: passed, 6 tests.
- `rtk err bun run --cwd platform/desktop build:extension eden`: passed.
- `rtk err bun run --cwd platform/desktop typecheck`: passed.
- Initial combined `bun test` command failed because browser-mode specs need Vitest Browser; rerun passed.
- Desloppify scan: expected exit 1 while repository findings remain; valid JSON saved to `desloppify-after.json`.
