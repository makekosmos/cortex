import { computed, ref, shallowRef, watch } from "vue";
import { statsApi } from "@/lib/api";
import type { PlaytimeStats } from "@/types";
import {
  addDays,
  buildMonthOptions,
  buildRecentDateOptions,
  formatDateLong,
  formatDuration,
  getMonthRange,
  getMonthValueFromRange,
  rangePresets,
  toHours,
  toIsoDate,
  type DailyTrendPoint,
  type PerGamePoint,
  type StatisticsRangePreset,
} from "@vue-app/lib/statistics";

const LOAD_ERROR_MESSAGE = "Не удалось загрузить статистику";

export function useStatisticsRange() {
  const today = new Date();
  const defaultEnd = toIsoDate(today);
  const defaultStart = toIsoDate(addDays(today, -29));
  const monthOptions = buildMonthOptions(today, 24);
  const dayOptions = buildRecentDateOptions(today, 365);

  const stats = ref<PlaytimeStats | null>(null);
  const loading = shallowRef(true);
  const error = shallowRef<string | null>(null);
  const rangePreset = shallowRef<StatisticsRangePreset>("30d");
  const startDate = shallowRef(defaultStart);
  const endDate = shallowRef(defaultEnd);
  const requestToken = shallowRef(0);

  const dailyData = computed<DailyTrendPoint[]>(() =>
    stats.value?.daily_totals.map((entry) => ({
      date: entry.date,
      hours: toHours(entry.seconds),
      seconds: entry.seconds,
    })) ?? [],
  );

  const perGameData = computed<PerGamePoint[]>(() =>
    stats.value?.per_game_totals.slice(0, 8).map((entry) => ({
      ...entry,
      hours: toHours(entry.seconds),
    })) ?? [],
  );

  const totalDays = computed(() => stats.value?.daily_totals.length ?? 0);
  const activeDays = computed(
    () => stats.value?.daily_totals.filter((entry) => entry.seconds > 0).length ?? 0,
  );
  const averageSeconds = computed(() => {
    if (!stats.value || totalDays.value <= 0) {
      return 0;
    }
    return Math.round(stats.value.total_seconds / totalDays.value);
  });
  const rangeLabel = computed(() =>
    stats.value
      ? `${formatDateLong(stats.value.range_start)} — ${formatDateLong(stats.value.range_end)}`
      : "",
  );
  const selectedMonthValue = computed(() =>
    getMonthValueFromRange(startDate.value, endDate.value),
  );
  const topGame = computed(() => stats.value?.per_game_totals[0] ?? null);
  const hasDailyData = computed(() => dailyData.value.some((entry) => entry.seconds > 0));
  const hasPerGameData = computed(() => perGameData.value.length > 0);
  const hasMoreGames = computed(
    () => (stats.value?.per_game_totals.length ?? 0) > perGameData.value.length,
  );
  const summary = computed(() => ({
    totalLabel: stats.value ? formatDuration(stats.value.total_seconds) : "—",
    averageLabel: stats.value ? formatDuration(averageSeconds.value) : "—",
    activeDaysLabel: stats.value
      ? `${activeDays.value} из ${totalDays.value} дней были активными`
      : "Нет данных",
    gamesCount: stats.value?.per_game_totals.length ?? 0,
    topGameLabel: topGame.value
      ? `Топ: ${topGame.value.name} — ${formatDuration(topGame.value.seconds)}`
      : "Нет активности",
  }));
  const perGameDescription = computed(() =>
    hasMoreGames.value ? "Показаны топ-8" : "За выбранный период",
  );

  const loadStats = async (start: string, end: string) => {
    const nextToken = requestToken.value + 1;
    requestToken.value = nextToken;
    loading.value = true;
    error.value = null;

    try {
      const response = await statsApi.getPlaytimeStats(start, end);
      if (requestToken.value !== nextToken) {
        return;
      }
      stats.value = response;
    } catch (cause) {
      console.error("Failed to load playtime stats:", cause);
      if (requestToken.value !== nextToken) {
        return;
      }
      error.value = LOAD_ERROR_MESSAGE;
    } finally {
      if (requestToken.value === nextToken) {
        loading.value = false;
      }
    }
  };

  watch(
    [startDate, endDate],
    ([nextStart, nextEnd]) => {
      void loadStats(nextStart, nextEnd);
    },
    { immediate: true },
  );

  const selectPreset = (presetId: StatisticsRangePreset) => {
    const preset = rangePresets.find((item) => item.id === presetId);
    if (!preset) {
      return;
    }

    const now = new Date();
    rangePreset.value = preset.id;
    endDate.value = toIsoDate(now);
    startDate.value = toIsoDate(addDays(now, -(preset.days - 1)));
  };

  const selectMonth = (value: string) => {
    const range = getMonthRange(value);
    if (!range) {
      return;
    }

    rangePreset.value = "month";
    startDate.value = range.start;
    endDate.value = range.end;
  };

  const updateStartDate = (value: string) => {
    rangePreset.value = "custom";
    startDate.value = value;
    if (value && value > endDate.value) {
      endDate.value = value;
    }
  };

  const updateEndDate = (value: string) => {
    rangePreset.value = "custom";
    endDate.value = value;
    if (value && value < startDate.value) {
      startDate.value = value;
    }
  };

  return {
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
  };
}
