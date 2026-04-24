<script setup lang="ts">
import CalendarShell from "@/components/calendar/CalendarShell.vue";
import { useCalendarState } from "@/composables/useCalendarState";
import { useSidebarState } from "@/composables/useSidebarState";

const {
  wrapClass,
  wrapStyle,
  titleWrapRef,
  titleGroupRef,
  titleGroupClass,
  titleGroupStyle,
  titleClass,
} = useSidebarState();

const calendarState = useCalendarState();
</script>

<template>
  <div class="flex min-h-0 w-full min-w-0 flex-1 flex-col">
    <div
      ref="titleWrapRef"
      :class="[wrapClass, 'flex min-h-8 items-center gap-2.5 px-7 pb-3 pt-6']"
      :style="wrapStyle"
    >
      <div
        ref="titleGroupRef"
        :class="[titleGroupClass, 'flex-wrap gap-3 whitespace-normal']"
        :style="titleGroupStyle"
      >
        <h1 :class="[titleClass, 'text-2xl font-bold text-(--foreground) select-none']">
          Календарь
        </h1>
      </div>
    </div>

    <CalendarShell
      :view-mode="calendarState.viewMode.value"
      :anchor-date="calendarState.anchorDate.value"
      :visible-days="calendarState.visibleDays.value"
      :range-label="calendarState.rangeLabel.value"
      @update:view-mode="calendarState.setViewMode"
      @previous="calendarState.goPrevious"
      @next="calendarState.goNext"
      @today="calendarState.goToday"
    />
  </div>
</template>
