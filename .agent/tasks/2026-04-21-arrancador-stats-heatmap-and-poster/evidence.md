# Evidence: Arrancador statistics heatmap, route cleanup, and Vue poster card

## Current Status
PASS

## Delivered scope

- The active Vue app no longer exposes Achievements or System in navigation:
  - `apps/arrancador/src-vue/components/AppSidebar.vue` no longer lists those sections
  - `apps/arrancador/src-vue/router.ts` redirects `/system` and `/achievements` to `/`
  - the old Vue route pages were removed from the active renderer tree
- The backend playtime stats crash was fixed in the Electron main process:
  - `apps/arrancador/electron/main/backend.ts` now calls `createPlaytimeStatsRepository(db, arkDbPath)` with the correct signature
  - the rebuilt `out/main/index.js` confirms the corrected call site
- The Vue statistics page was replaced with a heatmap-based implementation:
  - `apps/arrancador/src-vue/pages/StatisticsPage.vue`
  - `apps/arrancador/src-vue/composables/useStatisticsHeatmap.ts`
  - `apps/arrancador/src-vue/components/statistics/StatisticsHeatmap.vue`
  - `apps/arrancador/src-vue/components/statistics/StatisticsDayDetails.vue`
  - `apps/arrancador/src-vue/lib/statistics.ts`
- The selected day defaults to today and per-day details are fetched separately from the same IPC endpoint.
- `GamePosterCard` was ported from React to Vue and is now used by the active Arrancador game card:
  - new shared component: `packages/kepler-visuals/components/GamePosterCard.vue`
  - shared exports updated in `packages/kepler-visuals/components/index.ts` and `packages/kepler-visuals/index.ts`
  - active app wrapper updated in `apps/arrancador/src-vue/components/GameCard.vue`
  - old React `GamePosterCard` implementation and old React Arrancador wrapper/test were removed

## Verification run

- `bun run typecheck` -> PASS
- `bun run test` -> PASS
- `bun run build:renderer` -> PASS
- `bun run build:main` -> PASS
- `out/main/index.js` inspection -> PASS for corrected playtime repository call site

## Acceptance Criteria status

- AC1: PASS
  - Achievements and System were removed from the active Vue navigation and no longer render as active runtime routes.
- AC2: PASS
  - The `db.all is not a function` failure was fixed in main-process code by correcting the stats repository factory call.
- AC3: PASS
  - The statistics screen now renders a playtime heatmap, selects today by default, allows day selection, and shows per-day hours and game breakdown.
- AC4: PASS
  - `GamePosterCard.tsx` was replaced by a Vue shared component and the active Arrancador Vue card now uses it.
- AC5: PASS
  - Fresh checks passed for the touched paths and the raw artifacts were refreshed.

## Raw Artifacts

- `raw/typecheck.txt`
- `raw/test.txt`
- `raw/build-renderer.txt`
- `raw/build-main.txt`
- `raw/backend-playtime-proof.txt`
- `raw/react-poster-removal.txt`
- `raw/router.txt`
- `raw/sidebar.txt`
