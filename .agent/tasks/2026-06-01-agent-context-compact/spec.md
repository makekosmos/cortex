# Agent context compact generation

## Goal

Make all generated agent instruction files in the repository compact by default, with detailed documentation kept in `docs-site/` and loaded only by pointer when relevant.

## Scope

- Audit every `AGENTS.md` and `CLAUDE.md` in the repository.
- Change `scripts/sync-agents-docs.mjs` so generated agent files are concise boot/context files, not inline copies of large docs.
- Keep full detailed documentation available in `docs-site/` and `docs-site/public/full-llms.txt`.
- Document the anti-bloat rule in docs maintenance guidance.

## Acceptance Criteria

- AC1: `rg --files -g "AGENTS.md" -g "CLAUDE.md"` is audited and every returned file is classified in `evidence.md` with line and byte counts.
- AC2: Root `AGENTS.md` and `CLAUDE.md` are generated from the compact source and are each no more than 220 lines and 32 KB.
- AC3: Per-area generated `AGENTS.md` files are no more than 180 lines and 24 KB each.
- AC4: `scripts/sync-agents-docs.mjs` enforces the budgets during `bun run docs:sync`.
- AC5: Per-area `AGENTS.md` files preserve critical local instructions and point to full source docs instead of inlining large global sections.
- AC6: `docs-site/agents/docs-maintenance.md` documents the compact-context contract and what must not be added to generated boot context.
- AC7: `bun run docs:sync`, `bun run docs:check`, and lint for `scripts/sync-agents-docs.mjs` pass.

## Non-goals

- Do not clean unrelated Markdown files in this task.
- Do not edit generated `AGENTS.md` files directly except through `bun run docs:sync`.
- Do not touch unrelated working tree changes.
