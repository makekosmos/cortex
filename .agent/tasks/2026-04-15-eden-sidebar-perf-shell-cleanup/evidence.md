# Evidence — Eden sidebar perf shell cleanup

## Scope

Relevant files in this perf/shell cleanup pass:

- `apps/eden/ts/src/App.vue`
- `apps/eden/ts/src/App.css`
- `apps/eden/ts/src/store/layout.ts`
- `apps/eden/ts/src/store/eden.ts`
- `apps/eden/ts/src/composables/useTitlebarSafeArea.ts`
- `apps/eden/ts/src/components/SearchOverlay.vue`
- `.omx/context/eden-sidebar-perf-shell-cleanup-20260415T142952Z.md`
- `.omx/plans/prd-eden-sidebar-perf-shell-cleanup.md`
- `.omx/plans/test-spec-eden-sidebar-perf-shell-cleanup.md`
- `.agent/tasks/2026-04-15-eden-sidebar-perf-shell-cleanup/spec.md`

## Cleanup plan executed

1. Simplify shell/CSS around the sidebar.
2. Reduce reactive churn near sidebar-related paths.
3. Remove legacy vault-sidebar runtime state from the active renderer path.
4. Reverify with lint/typecheck/build/e2e.

## Simplifications made

- Removed heavy `app-main` shell effects that sat next to the sidebar:
  - dropped `backdrop-filter`
  - dropped `app-main::before` gradient overlay
  - removed transition on shell padding
- Deleted large unused/legacy vault-sidebar CSS blocks from `App.css` that no longer matched the current single-sidebar UI path.
- Removed renderer-side legacy vault-sidebar state from `layout.ts` and stopped hydrating it from `eden.ts`.
- Replaced full-array recent-entry sorting in `App.vue` with a bounded top-N selection helper instead of `copy + sort + slice` on every recompute.
- Narrowed `SearchOverlay` props so it receives `entryTitles` lookup data instead of the full `entries` array.
- Replaced `watchEffect` in `useTitlebarSafeArea.ts` with explicit watched dependencies and immediate update.

## Verification

### Commands

- `bun run lint` ✅
- `bunx tsc --noEmit -p tsconfig.json` ✅
- `bun run build` ✅
- `bunx playwright test tests/app.spec.ts -g "open an existing note|spaces, settings screen|single shared sidebar|settings sidebar visible" --config playwright.config.ts` ✅
- `bun run test:e2e` ✅ `19 passed`

## Acceptance mapping

- AC1: PASS — shell/CSS around sidebar is lighter (`App.css` simplified, heavy app-main effects removed).
- AC2: PASS — recent sidebar derivation is cheaper and overlay prop flow is narrower.
- AC3: PASS — legacy vault-sidebar runtime state removed from active renderer path (`layout.ts`, `eden.ts`).
- AC4: PASS — lint/typecheck/build/e2e all green.

## Remaining risks

- Main-process sidebar persistence still keeps legacy `vault` config compatibility in `main/store.ts` / preload / API shape. This is non-blocking and may be intentionally preserved for compatibility, but it means the old model is not fully deleted end-to-end.
- `SearchOverlay` itself still has heavy visual effects when opened; this pass targeted always-present sidebar shell cost first.
