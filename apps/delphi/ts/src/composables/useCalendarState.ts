import { computed, shallowRef } from "vue";
import {
  buildMonthGridDays,
  buildVisibleDays,
  formatRangeLabel,
  getVisibleRange,
  shiftAnchorDate,
  startOfDay,
  toRange,
} from "@/services/google-calendar/date";
import type { GoogleCalendarViewMode } from "@/services/google-calendar/contracts";

const STORAGE_KEY = "delphi-calendar-view-mode";

function loadInitialViewMode(): GoogleCalendarViewMode {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (
      raw === "day" ||
      raw === "four-days" ||
      raw === "week" ||
      raw === "month"
    ) {
      return raw;
    }
  } catch {
    // ignore
  }
  return "week";
}

export function useCalendarState() {
  const viewMode = shallowRef<GoogleCalendarViewMode>(loadInitialViewMode());
  const anchorDate = shallowRef(startOfDay(new Date()));

  const visibleDays = computed(() =>
    viewMode.value === "month"
      ? buildMonthGridDays(anchorDate.value)
      : buildVisibleDays(viewMode.value, anchorDate.value),
  );

  const visibleRange = computed(() => {
    const range = getVisibleRange(viewMode.value, anchorDate.value);
    return {
      ...range,
      request: toRange(range.start, range.end),
    };
  });

  const rangeLabel = computed(() =>
    formatRangeLabel(viewMode.value, anchorDate.value),
  );

  function setViewMode(next: GoogleCalendarViewMode) {
    viewMode.value = next;
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // ignore
    }
  }

  function goToday() {
    anchorDate.value = startOfDay(new Date());
  }

  function goPrevious() {
    anchorDate.value = startOfDay(
      shiftAnchorDate(anchorDate.value, viewMode.value, -1),
    );
  }

  function goNext() {
    anchorDate.value = startOfDay(
      shiftAnchorDate(anchorDate.value, viewMode.value, 1),
    );
  }

  return {
    viewMode,
    anchorDate,
    visibleDays,
    visibleRange,
    rangeLabel,
    setViewMode,
    goToday,
    goPrevious,
    goNext,
  };
}
