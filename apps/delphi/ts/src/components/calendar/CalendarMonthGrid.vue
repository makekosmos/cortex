<script setup lang="ts">
import { computed } from "vue";
import type { CalendarSurfaceEntry } from "@/services/calendar/contracts";
import {
  capitalize,
  entryIntersectsDay,
  formatEntryTime,
  isSameDay,
  startOfMonth,
  toDayKey,
  weekdayShortLabel,
} from "@/services/calendar/date";

const props = defineProps<{
  anchorDate: Date;
  days: Date[];
  entries: CalendarSurfaceEntry[];
}>();

const weekdayHeaders = Array.from({ length: 7 }, (_, index) =>
  weekdayShortLabel(props.days[index] ?? new Date(2026, 0, index + 5)),
);

const entriesByDay = computed(() => {
  const map = new Map<string, CalendarSurfaceEntry[]>();

  for (const day of props.days) {
    const dayKey = toDayKey(day);
    const dayEntries = props.entries
      .filter((entry) => entryIntersectsDay(entry, day))
      .toSorted((left, right) => {
        if (left.isAllDay !== right.isAllDay) return left.isAllDay ? -1 : 1;
        return left.title.localeCompare(right.title, "ru");
      });
    map.set(dayKey, dayEntries);
  }

  return map;
});

function dayEntries(day: Date) {
  return entriesByDay.value.get(toDayKey(day)) ?? [];
}

function chipStyle(entry: CalendarSurfaceEntry) {
  return {
    borderColor: entry.color ?? "#2563eb",
  };
}

function openLink(entry: CalendarSurfaceEntry) {
  if (!entry.link) return;
  window.open(entry.link, "_blank", "noopener,noreferrer");
}

const currentMonth = computed(() => startOfMonth(props.anchorDate).getMonth());
</script>

<template>
  <div class="overflow-auto rounded-2xl border border-(--border) bg-(--background)">
    <div class="grid min-w-[900px] grid-cols-7 border-b border-(--border)">
      <div
        v-for="header in weekdayHeaders"
        :key="header"
        class="border-r border-(--border) px-4 py-3 text-xs font-semibold uppercase tracking-[0.08em] text-(--muted-foreground) last:border-r-0"
      >
        {{ capitalize(header) }}
      </div>
    </div>

    <div class="grid min-w-[900px] grid-cols-7">
      <div
        v-for="day in days"
        :key="toDayKey(day)"
        class="min-h-36 border-b border-r border-(--border) p-3 last:border-r-0"
        :class="day.getMonth() === currentMonth ? 'bg-(--background)' : 'bg-(--secondary)/25'"
      >
        <div class="mb-2 flex items-center justify-between">
          <span
            class="inline-flex h-7 min-w-7 items-center justify-center rounded-full px-2 text-sm font-semibold"
            :class="
              isSameDay(day, new Date())
                ? 'bg-(--foreground) text-(--background)'
                : 'text-(--foreground)'
            "
          >
            {{ day.getDate() }}
          </span>

          <span
            v-if="dayEntries(day).length > 0"
            class="text-[11px] text-(--muted-foreground)"
          >
            {{ dayEntries(day).length }}
          </span>
        </div>

        <div v-if="dayEntries(day).length === 0" class="text-xs text-(--muted-foreground)/50">
          Пусто
        </div>

        <div v-else class="flex flex-col gap-1.5">
          <button
            v-for="entry in dayEntries(day).slice(0, 4)"
            :key="entry.id"
            type="button"
            class="rounded-lg border-l-4 bg-(--secondary)/70 px-2 py-1 text-left transition-colors hover:bg-(--secondary)"
            :style="chipStyle(entry)"
            @click="openLink(entry)"
          >
            <div class="truncate text-xs font-medium text-(--foreground)">
              {{ entry.title }}
            </div>
            <div class="truncate text-[11px] text-(--muted-foreground)">
              {{ formatEntryTime(entry, day) }}
            </div>
          </button>

          <div
            v-if="dayEntries(day).length > 4"
            class="text-[11px] text-(--muted-foreground)"
          >
            +{{ dayEntries(day).length - 4 }} ещё
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
