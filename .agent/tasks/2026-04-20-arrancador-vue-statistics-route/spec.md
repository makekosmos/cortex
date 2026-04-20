# Task: Arrancador Vue statistics route

## Summary
Port the current React `Statistics` page into the Vue/Vapor renderer under `apps/arrancador/src-vue/**`, preserving the existing statistics workflow and keeping the React `src/` tree untouched.

## Scope And Constraints

- Edit only `apps/arrancador/src-vue/**` for product code, plus task-proof artifacts under `.agent/tasks/2026-04-20-arrancador-vue-statistics-route/`.
- Reuse existing renderer APIs/types from `apps/arrancador/src/**` where needed, but do not modify React implementation files.
- Prefer Composition API with `<script setup lang="ts">`.
- Prefer a dependency-free chart solution using SVG/CSS unless a dependency is strictly necessary.

## Component Map

- `pages/StatisticsPage.vue`
  Responsibility: thin route composition surface that binds the statistics composable to the feature UI.
- `composables/useStatisticsRange.ts`
  Responsibility: own date preset/range state, load stats from `statsApi`, and expose typed derived values for the page.
- `components/statistics/StatisticsRangePanel.vue`
  Responsibility: render preset buttons and month/day selectors, emitting explicit range changes upward.
  Props: current preset, selected month, start date, end date, month options, day options, range label.
  Emits: `select-preset`, `select-month`, `update-start-date`, `update-end-date`.
- `components/statistics/StatisticsSummaryCards.vue`
  Responsibility: render the summary KPI cards from already-derived statistics values.
  Props: total label, average label, active day copy, game count, top game copy.
- `components/statistics/StatisticsDailyTrendChart.vue`
  Responsibility: render the daily trend chart or empty state from precomputed daily series data.
  Props: data, hasData, ariaLabel.
- `components/statistics/StatisticsPerGameChart.vue`
  Responsibility: render the per-game breakdown chart/list or empty state from precomputed per-game data.
  Props: data, hasData, description, ariaLabel.

## Acceptance Criteria

- AC1: `/statistics` in the Vue router renders a real statistics page instead of the migration placeholder.
- AC2: The Vue statistics page loads playtime data through `statsApi.getPlaytimeStats(startDate, endDate)` and updates when presets, month selection, or custom day bounds change.
- AC3: The page renders the same major information architecture as React: range controls, summary cards, daily trend, and per-game breakdown, with loading/error/empty states.
- AC4: The implementation stays inside `apps/arrancador/src-vue/**` for product code and uses Composition API `<script setup lang="ts">` with focused components/composables rather than one large page file.
- AC5: Verification covers the Vue statistics behavior through runnable checks, and evidence artifacts record the current code/results for each acceptance criterion.
