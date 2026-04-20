<script setup lang="ts">
import { Loader2 } from "lucide-vue-next";
import StatisticsDailyTrendChart from "../components/statistics/StatisticsDailyTrendChart.vue";
import StatisticsPerGameChart from "../components/statistics/StatisticsPerGameChart.vue";
import StatisticsRangePanel from "../components/statistics/StatisticsRangePanel.vue";
import StatisticsSummaryCards from "../components/statistics/StatisticsSummaryCards.vue";
import { useStatisticsRange } from "../composables/useStatisticsRange";

const {
  stats,
  loading,
  error,
  rangePreset,
  rangePresets,
  startDate,
  endDate,
  monthOptions,
  dayOptions,
  selectedMonthValue,
  rangeLabel,
  dailyData,
  perGameData,
  hasDailyData,
  hasPerGameData,
  perGameDescription,
  summary,
  selectPreset,
  selectMonth,
  updateStartDate,
  updateEndDate,
} = useStatisticsRange();
</script>

<template>
  <div v-if="loading && !stats" class="flex h-full items-center justify-center p-6">
    <Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
  </div>

  <div
    v-else-if="!stats && error"
    class="flex h-full items-center justify-center p-6"
  >
    <div class="text-sm text-red-300">{{ error }}</div>
  </div>

  <div v-else class="mx-auto max-w-7xl space-y-6 p-4 sm:p-6">
    <div class="space-y-2">
      <h1 class="text-2xl font-semibold tracking-tight">Статистика</h1>
      <p class="text-sm text-muted-foreground">
        Обзор игровой активности за выбранный период
      </p>
    </div>

    <StatisticsRangePanel
      :range-preset="rangePreset"
      :range-label="rangeLabel"
      :selected-month-value="selectedMonthValue"
      :start-date="startDate"
      :end-date="endDate"
      :month-options="monthOptions"
      :day-options="dayOptions"
      :presets="rangePresets"
      @select-preset="selectPreset"
      @select-month="selectMonth"
      @update-start-date="updateStartDate"
      @update-end-date="updateEndDate"
    />

    <div
      v-if="error"
      class="rounded-2xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300"
    >
      {{ error }}
    </div>

    <StatisticsSummaryCards
      :total-label="summary.totalLabel"
      :range-label="rangeLabel"
      :average-label="summary.averageLabel"
      :active-days-label="summary.activeDaysLabel"
      :games-count="summary.gamesCount"
      :top-game-label="summary.topGameLabel"
    />

    <div class="grid gap-4 xl:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
      <StatisticsDailyTrendChart
        :data="dailyData"
        :has-data="hasDailyData"
        :range-label="rangeLabel"
        :aria-label="
          hasDailyData
            ? `Диаграмма динамики по дням: ${dailyData.length} точек`
            : 'Нет данных за период'
        "
      />
      <StatisticsPerGameChart
        :data="perGameData"
        :has-data="hasPerGameData"
        :description="perGameDescription"
        :aria-label="
          hasPerGameData
            ? `Разбивка по играм: ${perGameData.length} игр`
            : 'Нет данных по играм'
        "
      />
    </div>
  </div>
</template>
