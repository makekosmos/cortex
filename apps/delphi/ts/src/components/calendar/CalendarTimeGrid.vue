<script setup lang="ts">
import { computed } from "vue";
import type { CalendarSurfaceEntry } from "@/services/google-calendar/contracts";
import {
  DAY_MS,
  addDays,
  entryIntersectsDay,
  formatEntryTime,
  getEntryDaySpan,
  isSameDay,
  startOfDay,
  toDayKey,
  weekdayLongLabel,
} from "@/services/google-calendar/date";

const props = defineProps<{
  days: Date[];
  entries: CalendarSurfaceEntry[];
}>();

type TimedLayoutEntry = CalendarSurfaceEntry & {
  top: number;
  height: number;
};

const PIXELS_PER_MINUTE = 1.1;
const TIMELINE_HEIGHT = 24 * 60 * PIXELS_PER_MINUTE;
const hours = Array.from({ length: 24 }, (_, hour) => hour);

function formatHourLabel(hour: number): string {
  return `${String(hour).padStart(2, "0")}:00`;
}

function entryAccent(entry: CalendarSurfaceEntry): string | undefined {
  return entry.color ?? (entry.source === "task" ? "#2563eb" : "#16a34a");
}

const entriesByDay = computed(() => {
  const map = new Map<string, { allDay: CalendarSurfaceEntry[]; timed: TimedLayoutEntry[] }>();

  for (const day of props.days) {
    const dayKey = toDayKey(day);
    const allDay: CalendarSurfaceEntry[] = [];
    const timed: TimedLayoutEntry[] = [];

    for (const entry of props.entries) {
      if (!entryIntersectsDay(entry, day)) continue;

      if (entry.isAllDay) {
        allDay.push(entry);
        continue;
      }

      const span = getEntryDaySpan(entry, day);
      const dayStart = startOfDay(day).getTime();
      const startMinutes = Math.max(
        0,
        Math.round((span.start.getTime() - dayStart) / 60000),
      );
      const durationMinutes = Math.max(
        30,
        Math.round((span.end.getTime() - span.start.getTime()) / 60000),
      );

      timed.push({
        ...entry,
        top: startMinutes * PIXELS_PER_MINUTE,
        height: durationMinutes * PIXELS_PER_MINUTE,
      });
    }

    allDay.sort((left, right) => left.title.localeCompare(right.title, "ru"));
    timed.sort((left, right) => {
      const startDelta =
        new Date(left.start).getTime() - new Date(right.start).getTime();
      if (startDelta !== 0) return startDelta;
      return left.title.localeCompare(right.title, "ru");
    });
    map.set(dayKey, { allDay, timed });
  }

  return map;
});

function dayEntries(day: Date) {
  return entriesByDay.value.get(toDayKey(day)) ?? { allDay: [], timed: [] };
}

function dayHeaderLabel(day: Date): string {
  const now = new Date();
  const base = `${weekdayLongLabel(day)}, ${day.getDate()}`;
  return isSameDay(day, now) ? `${base} · Сегодня` : base;
}

function eventStyle(entry: TimedLayoutEntry) {
  return {
    top: `${entry.top}px`,
    height: `${entry.height}px`,
    borderLeftColor: entryAccent(entry),
  };
}

function chipStyle(entry: CalendarSurfaceEntry) {
  return {
    borderColor: entryAccent(entry),
  };
}

function openLink(entry: CalendarSurfaceEntry) {
  if (!entry.link) return;
  window.open(entry.link, "_blank", "noopener,noreferrer");
}

function spansIntoPreviousDay(entry: CalendarSurfaceEntry, day: Date): boolean {
  if (entry.isAllDay) return false;
  return new Date(entry.start).getTime() < startOfDay(day).getTime();
}

function spansIntoNextDay(entry: CalendarSurfaceEntry, day: Date): boolean {
  if (entry.isAllDay) return false;
  return new Date(entry.end).getTime() > addDays(startOfDay(day), 1).getTime();
}
</script>

<template>
  <div class="overflow-auto rounded-2xl border border-(--border) bg-(--background)">
    <div
      class="grid min-w-[760px]"
      :style="{
        gridTemplateColumns: `64px repeat(${days.length}, minmax(220px, 1fr))`,
      }"
    >
      <div class="border-b border-r border-(--border) bg-(--background) px-3 py-3" />

      <div
        v-for="day in days"
        :key="toDayKey(day)"
        class="border-b border-(--border) px-4 py-3"
      >
        <div class="text-sm font-semibold text-(--foreground)">
          {{ dayHeaderLabel(day) }}
        </div>
      </div>

      <div class="border-r border-(--border) bg-(--secondary)/40 px-3 py-3 text-xs font-medium text-(--muted-foreground)">
        Весь день
      </div>

      <div
        v-for="day in days"
        :key="`${toDayKey(day)}-all-day`"
        class="min-h-20 border-b border-(--border) px-3 py-3"
      >
        <div v-if="dayEntries(day).allDay.length === 0" class="text-xs text-(--muted-foreground)/60">
          Нет событий
        </div>

        <div v-else class="flex flex-col gap-2">
          <button
            v-for="entry in dayEntries(day).allDay"
            :key="entry.id"
            type="button"
            class="rounded-xl border-l-4 border px-3 py-2 text-left transition-colors hover:bg-(--secondary)"
            :style="chipStyle(entry)"
            @click="openLink(entry)"
          >
            <div class="truncate text-sm font-medium text-(--foreground)">
              {{ entry.title }}
            </div>
            <div
              v-if="entry.subtitle"
              class="truncate text-xs text-(--muted-foreground)"
            >
              {{ entry.subtitle }}
            </div>
          </button>
        </div>
      </div>

      <div class="border-r border-(--border) bg-(--secondary)/40">
        <div
          v-for="hour in hours"
          :key="hour"
          class="border-b border-(--border) px-3 pt-2 text-xs text-(--muted-foreground)"
          :style="{ height: `${60 * PIXELS_PER_MINUTE}px` }"
        >
          {{ formatHourLabel(hour) }}
        </div>
      </div>

      <div
        v-for="day in days"
        :key="`${toDayKey(day)}-timeline`"
        class="relative border-r border-(--border) last:border-r-0"
        :style="{ height: `${TIMELINE_HEIGHT}px` }"
      >
        <div
          v-for="hour in hours"
          :key="`${toDayKey(day)}-${hour}`"
          class="border-b border-dashed border-(--border)/80"
          :style="{ height: `${60 * PIXELS_PER_MINUTE}px` }"
        />

        <button
          v-for="entry in dayEntries(day).timed"
          :key="entry.id"
          type="button"
          class="absolute left-1.5 right-1.5 overflow-hidden rounded-xl border-l-4 bg-(--secondary)/90 px-2 py-1.5 text-left shadow-sm transition-colors hover:bg-(--secondary)"
          :style="eventStyle(entry)"
          @click="openLink(entry)"
        >
          <div class="truncate text-xs font-semibold text-(--foreground)">
            {{ entry.title }}
          </div>
          <div class="truncate text-[11px] text-(--muted-foreground)">
            {{ formatEntryTime(entry, day) }}
          </div>
          <div
            v-if="entry.subtitle || spansIntoPreviousDay(entry, day) || spansIntoNextDay(entry, day)"
            class="truncate text-[11px] text-(--muted-foreground)/80"
          >
            <span v-if="spansIntoPreviousDay(entry, day)">← </span>
            <span v-if="entry.subtitle">{{ entry.subtitle }}</span>
            <span v-if="spansIntoNextDay(entry, day)"> →</span>
          </div>
        </button>
      </div>
    </div>
  </div>
</template>
