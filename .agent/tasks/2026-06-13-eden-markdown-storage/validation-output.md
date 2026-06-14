# Validation output summaries

- `rtk bun test products/eden/tests/content.test.ts products/eden/tests/cmGate.test.ts products/eden/tests/obsidianVault.test.ts` — PASSED (24 pass / 0 fail).
- `rtk bun test products/eden/tests/components/CmConvert.spec.ts products/eden/tests/content.test.ts` — PASSED (5 pass / 0 fail).
- `rtk bun run products:build` — PASSED; Eden extension built successfully after TipTap removal.
- `rtk bun run docs:check` — PASSED; no stale docs references.
- `rtk grep -R "@tiptap\|mdConvert\|createMdConverter" -n products/eden/src products/eden/tests products/eden/package.json bun.lock || true` — PASSED; no matches.
- Broader Eden browser component suite still has known unrelated stale failures, so it was not required for this focused storage proof pass.
