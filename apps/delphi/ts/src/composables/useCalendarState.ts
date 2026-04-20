import { computed, shallowRef } from "vue";
import {
  buildMonthGridDays,
  buildVisibleDays,
  formatRangeLabel,
  shiftAnchorDate,
  startOfDay,
} from "@/services/calendar/date";
import type { CalendarViewMode } from "@/services/calendar/contracts";

const STORAGE_KEY = "delphi-calendar-view-mode";

function loadInitialViewMode(): CalendarViewMode {
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
  const viewMode = shallowRef<CalendarViewMode>(loadInitialViewMode());
  const anchorDate = shallowRef(startOfDay(new Date()));

  const visibleDays = computed(() =>
    viewMode.value === "month"
      ? buildMonthGridDays(anchorDate.value)
      : buildVisibleDays(viewMode.value, anchorDate.value),
  );

  const rangeLabel = computed(() =>
    formatRangeLabel(viewMode.value, anchorDate.value),
  );

  function setViewMode(next: CalendarViewMode) {
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
    rangeLabel,
    setViewMode,
    goToday,
    goPrevious,
    goNext,
  };
}
