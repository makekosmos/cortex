# Evidence — Eden writer-first performance + zen mode

## Scope
- `apps/eden/ts/src/App.vue`
- `apps/eden/ts/src/Editor.vue`
- `apps/eden/ts/src/composables/useKeyboard.ts`
- `apps/eden/ts/src/store/layout.ts`
- `apps/eden/ts/src/vite-env.d.ts`
- `apps/eden/ts/tests/app.spec.ts`
- `.agent/tasks/2026-04-14-eden-writer-performance/spec.md`

## What changed
- Added a real **zen mode runtime policy**: hotkey toggle, shell unmounting, search shutdown, editor exit control.
- Reduced editor hot-path pressure by replacing full snapshot-based dirty detection with **revision tracking + persisted-equivalence reconciliation**.
- Guarded against external `content_json` rehydrate overwriting locally dirty editor state.
- Exposed renderer perf summaries through `window.__edenPerf`.
- Added E2E coverage for:
  - normal + zen typing metrics
  - composition-like input smoke
  - delayed-save navigation stability
  - duplicate-title failure and retry recovery
- Saved raw perf metrics artifact from Playwright.

## Verification
### Fresh commands
- `bun run lint` ✅
- `bun run test:e2e` ✅ (`17 passed`; includes `bun run build`)
- Architect re-review ✅ APPROVE

### Raw artifacts
- `artifacts/lint.txt`
- `artifacts/test-e2e.txt`
- `artifacts/typing-perf-playwright.json`

## Perf artifact summary
From `artifacts/typing-perf-playwright.json`:
- Normal mode input-to-next-paint p95/p99: `16.2ms / 16.9ms`
- Zen mode input-to-next-paint p95/p99: `15.8ms / 16.5ms`
- Normal mode update-to-next-paint p95/p99: `15.0ms / 15.7ms`
- Zen mode update-to-next-paint p95/p99: `14.8ms / 15.5ms`
- Long tasks observed: `0`

## Architect verification
Latest architect verdict: **APPROVE**
- zen mode is runtime isolation, not CSS-only
- save correctness / external sync protections are acceptable
- verification evidence is sufficient for Ralph completion

## Deslop pass
Scoped `ai-slop-cleaner` review was run on Ralph-owned files only.
Result: **no additional safe cleanup edits were required** beyond the existing bounded implementation.

## Remaining risks
- `npx tsc --noEmit` is not independently usable in this monorepo due duplicate workspace package names outside the app package; build/test still pass because app-local `tsc` runs inside `bun run build`.
- The composition coverage is a smoke test for composition-like events rather than a full OS-level IME integration test.

## Fresh post-hook verification
- `bun run lint` ✅ (see `artifacts/lint-final.txt`)
- Focused Playwright re-run for new Ralph-added scenarios ✅ `4 passed` (see `artifacts/playwright-focused-final.txt`)
