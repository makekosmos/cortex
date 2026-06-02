# Command Results

- `bun run --cwd extensions/eden test:unit` — PASS, 20 tests.
- `bun run --cwd extensions/eden test:vue` — PASS, 6 files / 22 tests.
- `bun run --cwd shell build:extensions` — PASS.
- `bun run --cwd shell build:extensions` after TaskRef `preventScroll` follow-up — PASS.
- `bun run --cwd shell typecheck` — PASS.
- `bun run --cwd shell typecheck` after TaskRef `preventScroll` follow-up — PASS.
- `bun run ark:guard:writes` — PASS.
- `bun run ark:smoke` — PASS, ARK smoke matrix passed.
- `node .agent/tasks/2026-05-28-eden-scroll-drag-select/smoke/verify-eden-scroll-drag-select.mjs` — PASS.
- `bun run docs:check` — PASS.
- `bun run docs:sync` — PASS.
- final `bun run docs:check` — PASS.

Browser plugin note: in-app browser Node runtime failed before tab creation twice with Windows sandbox setup failure. Fallback Playwright was used only for local smoke verification.
