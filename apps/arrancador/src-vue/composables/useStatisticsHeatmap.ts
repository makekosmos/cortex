import {
  addDays,
  buildHeatmapWeeks,
  formatDateLong,
  formatDateWithWeekday,
  formatDuration,
  formatHours,
  HEATMAP_RANGE_DAYS,
  toIsoDate,
} from "@vue-app/lib/statistics";
import { computed, shallowRef, watch } from "vue";
import { statsApi } from "@/lib/api";
import type { PlaytimeStats } from "@/types";

const RANGE_ERROR_MESSAGE = "Не удалось загрузить тепловую карту статистики";
const DAY_ERROR_MESSAGE = "Не удалось загрузить статистику по выбранному дню";

export function useStatisticsHeatmap() {
  const today = new Date();
  const todayDate = toIsoDate(today);
  const rangeEnd = shallowRef(todayDate);
  const rangeStart = shallowRef(toIsoDate(addDays(today, -(HEATMAP_RANGE_DAYS - 1))));
  const selectedDate = shallowRef(todayDate);

  const rangeStats = shallowRef<PlaytimeStats | null>(null);
  const dayStats = shallowRef<PlaytimeStats | null>(null);

  const isRangeLoading = shallowRef(false);
  const isDayLoading = shallowRef(false);
  const rangeError = shallowRef<string | null>(null);
  const dayError = shallowRef<string | null>(null);
  const rangeRequestToken = shallowRef(0);
  const dayRequestToken = shallowRef(0);

  const isInitialLoading = computed(
    () => (isRangeLoading.value && !rangeStats.value) || (isDayLoading.value && !dayStats.value),
  );

  const heatmapWeeks = computed(() =>
    buildHeatmapWeeks(
      rangeStats.value?.daily_totals ?? [],
      rangeStart.value,
      rangeEnd.value,
      selectedDate.value,
      todayDate,
    ),
  );

  const selectedDayRows = computed(
    () =>
      dayStats.value?.per_game_totals.map((entry) => ({
        ...entry,
        hours: Math.round((entry.seconds / 3600) * 10) / 10,
      })) ?? [],
  );

  const selectedDayLabel = computed(() => formatDateWithWeekday(selectedDate.value));
  const selectedDayTotalLabel = computed(() =>
    dayStats.value ? formatHours(dayStats.value.total_seconds) : "0.0 ч",
  );
  const selectedDayDurationLabel = computed(() =>
    dayStats.value ? formatDuration(dayStats.value.total_seconds) : "0 мин",
  );
  const heatmapTotalLabel = computed(() =>
    rangeStats.value ? formatDuration(rangeStats.value.total_seconds) : "0 мин",
  );
  const heatmapRangeLabel = computed(
    () => `${formatDateLong(rangeStart.value)} — ${formatDateLong(rangeEnd.value)}`,
  );

  async function loadRange(start: string, end: string) {
    const nextToken = rangeRequestToken.value + 1;
    rangeRequestToken.value = nextToken;
    isRangeLoading.value = true;
    rangeError.value = null;

    try {
      const response = await statsApi.getPlaytimeStats(start, end);
      if (rangeRequestToken.value !== nextToken) {
        return;
      }
      rangeStats.value = response;
    } catch (cause) {
      console.error("Failed to load statistics heatmap:", cause);
      if (rangeRequestToken.value !== nextToken) {
        return;
      }
      rangeError.value = RANGE_ERROR_MESSAGE;
    } finally {
      if (rangeRequestToken.value === nextToken) {
        isRangeLoading.value = false;
      }
    }
  }

  async function loadDay(date: string) {
    const nextToken = dayRequestToken.value + 1;
    dayRequestToken.value = nextToken;
    isDayLoading.value = true;
    dayError.value = null;

    try {
      const response = await statsApi.getPlaytimeStats(date, date);
      if (dayRequestToken.value !== nextToken) {
        return;
      }
      dayStats.value = response;
    } catch (cause) {
      console.error("Failed to load selected day statistics:", cause);
      if (dayRequestToken.value !== nextToken) {
        return;
      }
      dayError.value = DAY_ERROR_MESSAGE;
    } finally {
      if (dayRequestToken.value === nextToken) {
        isDayLoading.value = false;
      }
    }
  }

  function selectDate(date: string) {
    if (date === selectedDate.value) {
      return;
    }
    selectedDate.value = date;
  }

  watch([rangeStart, rangeEnd], ([nextStart, nextEnd]) => {
    void loadRange(nextStart, nextEnd);
  }, { immediate: true });

  watch(selectedDate, (date) => {
    void loadDay(date);
  }, { immediate: true });

  return {
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
  };
}
