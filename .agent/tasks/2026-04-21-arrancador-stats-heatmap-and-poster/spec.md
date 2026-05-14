# Task: Arrancador statistics heatmap, route cleanup, and Vue poster card

## Summary

Update the active Vue renderer in `apps/arrancador` to remove the Achievements/System sections from the app, repair playtime statistics end-to-end, replace the current statistics page with a day-selectable heatmap experience, and port `GamePosterCard.tsx` to Vue for use in the active Arrancador UI.

## Component Map

- `src-vue/pages/StatisticsPage.vue`
  - Route-level composition surface for the statistics screen.
- `src-vue/composables/useStatisticsHeatmap.ts`
  - Single source of truth for heatmap range data, selected day, selected-day details, loading, and errors.
- `src-vue/components/statistics/StatisticsHeatmap.vue`
  - Presentational heatmap grid; receives prepared cells and emits selected date.
- `src-vue/components/statistics/StatisticsDayDetails.vue`
  - Presentational selected-day summary and per-game breakdown.
- `packages/kosmos-visuals/components/GamePosterCard.vue`
  - Reusable poster card visual primitive for Vue consumers.
- `src-vue/components/GameCard.vue`
  - Thin Arrancador adapter that maps `Game` data into the shared Vue poster card.

## Acceptance Criteria

- AC1: The active Vue app no longer exposes Achievements or System sections in sidebar navigation, and old `/achievements` and `/system` routes no longer render those pages in the active runtime.
- AC2: `get_playtime_stats` works again in the Electron backend; the `db.all is not a function` failure is removed by a code fix rather than being papered over in the renderer.
- AC3: The Vue statistics page renders a heatmap based on playtime hours, selects today's date by default, lets the user choose another day, and shows per-day stats for the selected date including played hours.
- AC4: `GamePosterCard.tsx` is ported to Vue, the active Arrancador Vue `GameCard` uses the Vue poster card, and the old React poster-card implementation is removed from the repo/runtime path.
- AC5: Verification artifacts show passing checks for the touched paths, or any remaining environment-only blocker is explicitly documented after revalidation.
