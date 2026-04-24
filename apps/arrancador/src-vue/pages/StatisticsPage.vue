<script setup lang="ts">
import { Loader2 } from "lucide-vue-next";
import StatisticsDayDetails from "../components/statistics/StatisticsDayDetails.vue";
import StatisticsHeatmap from "../components/statistics/StatisticsHeatmap.vue";
import { useStatisticsHeatmap } from "../composables/useStatisticsHeatmap";

const {
  dayError,
  heatmapRangeLabel,
  heatmapTotalLabel,
  heatmapWeeks,
  isDayLoading,
  isInitialLoading,
  isRangeLoading,
  rangeError,
  selectedDate,
  selectedDayDurationLabel,
  selectedDayLabel,
  selectedDayRows,
  selectedDayTotalLabel,
  selectDate,
} = useStatisticsHeatmap();
</script>

<template>
  <div v-if="isInitialLoading" class="flex h-full items-center justify-center p-6">
    <Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
  </div>

  <div v-else class="mx-auto max-w-7xl space-y-6 p-4 sm:p-6">
    <div class="space-y-2">
      <h1 class="text-2xl font-semibold tracking-tight">Статистика</h1>
      <p class="text-sm text-muted-foreground">
        Тепловая карта по количеству наигранных часов. По умолчанию выбран сегодняшний день.
      </p>
    </div>

    <div
      v-if="rangeError"
      class="rounded-2xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300"
    >
      {{ rangeError }}
    </div>

    <StatisticsHeatmap
      :range-label="heatmapRangeLabel"
      :total-label="heatmapTotalLabel"
      :weeks="heatmapWeeks"
      :selected-date="selectedDate"
      :loading="isRangeLoading"
      @select-date="selectDate"
    />

    <StatisticsDayDetails
      :selected-date-label="selectedDayLabel"
      :total-label="selectedDayTotalLabel"
      :duration-label="selectedDayDurationLabel"
      :loading="isDayLoading"
      :error="dayError"
      :rows="selectedDayRows"
    />
  </div>
</template>
