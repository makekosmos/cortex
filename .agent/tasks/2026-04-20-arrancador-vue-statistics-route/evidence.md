# Evidence: Arrancador Vue statistics route

## Summary

The Vue renderer now has a real `/statistics` route backed by a Composition API feature implementation under `apps/arrancador/src-vue/**`. The page loads `statsApi` data for the selected range, exposes preset/month/custom date controls, renders summary cards, and shows daily/per-game visual breakdowns without adding a chart dependency.

## Acceptance Criteria

### AC1 - PASS

- `apps/arrancador/src-vue/router.ts:8` imports `StatisticsPage`.
- `apps/arrancador/src-vue/router.ts:62` maps `path: "statistics"` to `component: StatisticsPage`.

### AC2 - PASS

- `apps/arrancador/src-vue/composables/useStatisticsRange.ts:22` introduces the statistics range composable.
- `apps/arrancador/src-vue/composables/useStatisticsRange.ts:116` watches `startDate` and `endDate` and reloads from `statsApi.getPlaytimeStats(start, end)`.
- `apps/arrancador/src-vue/composables/useStatisticsRange.ts:124`, `:136`, `:147`, and `:155` implement preset, month, start-date, and end-date updates.
- `apps/arrancador/src-vue/components/statistics/StatisticsRangePanel.vue:67`, `:83`, and `:98` render the month/start/end selectors used by the route.
- `apps/arrancador/src-vue/test/statistics-page.test.ts:50-79` verifies the initial load plus preset/month/custom range reloads.

### AC3 - PASS

- `apps/arrancador/src-vue/pages/StatisticsPage.vue:35-94` renders the page shell, range controls, inline error banner, summary cards, daily trend, and per-game breakdown.
- `apps/arrancador/src-vue/components/statistics/StatisticsSummaryCards.vue:16-38` renders the three summary KPI cards.
- `apps/arrancador/src-vue/components/statistics/StatisticsDailyTrendChart.vue:108-216` renders the SVG daily trend chart plus empty state.
- `apps/arrancador/src-vue/components/statistics/StatisticsPerGameChart.vue:23-59` renders the per-game breakdown plus empty state.
- `apps/arrancador/src-vue/test/statistics-page.test.ts:82-89` verifies the initial error state.

### AC4 - PASS

- `apps/arrancador/src-vue/pages/StatisticsPage.vue:1-31` is a thin route composition surface built with `<script setup lang="ts">`.
- `apps/arrancador/src-vue/composables/useStatisticsRange.ts`, `apps/arrancador/src-vue/lib/statistics.ts`, and the four files in `apps/arrancador/src-vue/components/statistics/` keep state/formatting/presentation split by responsibility.
- No React `apps/arrancador/src/**` files were modified.

### AC5 - PASS

- `bun run typecheck` passed.
  - Raw output: `.agent/tasks/2026-04-20-arrancador-vue-statistics-route/raw/typecheck.txt`
- `bun x vitest run --configLoader native --config src-vue/test/vitest.config.mjs` passed with `2` tests.
  - Raw output: `.agent/tasks/2026-04-20-arrancador-vue-statistics-route/raw/vitest-vue-statistics.txt`
- Verification artifacts for this task are present in `.agent/tasks/2026-04-20-arrancador-vue-statistics-route/`.

## Additional Checks

- `bun run build:renderer:vue`
  - Status: BLOCKED
  - Evidence: `.agent/tasks/2026-04-20-arrancador-vue-statistics-route/raw/build-renderer-vue.txt`
  - Notes: the command fails while loading existing Tailwind/Vite native dependencies (`@tailwindcss/oxide-win32-x64-msvc`) and reports `spawn EPERM` before app compilation.

## Changed Product Files

- `apps/arrancador/src-vue/router.ts`
- `apps/arrancador/src-vue/pages/StatisticsPage.vue`
- `apps/arrancador/src-vue/composables/useStatisticsRange.ts`
- `apps/arrancador/src-vue/lib/statistics.ts`
- `apps/arrancador/src-vue/components/statistics/StatisticsRangePanel.vue`
- `apps/arrancador/src-vue/components/statistics/StatisticsSummaryCards.vue`
- `apps/arrancador/src-vue/components/statistics/StatisticsDailyTrendChart.vue`
- `apps/arrancador/src-vue/components/statistics/StatisticsPerGameChart.vue`
- `apps/arrancador/src-vue/test/statistics-page.test.ts`
- `apps/arrancador/src-vue/test/vitest.config.mjs`
