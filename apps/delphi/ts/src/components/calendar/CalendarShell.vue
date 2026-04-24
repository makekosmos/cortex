<script setup lang="ts">
import { computed } from "vue";
import { ChevronLeft, ChevronRight } from "lucide-vue-next";
import { storeToRefs } from "pinia";
import CalendarMonthGrid from "@/components/calendar/CalendarMonthGrid.vue";
import CalendarTimeGrid from "@/components/calendar/CalendarTimeGrid.vue";
import CalendarViewSwitch from "@/components/calendar/CalendarViewSwitch.vue";
import { useSidebarState } from "@/composables/useSidebarState";
import { useTodoStore } from "@/store/todos";
import type {
  CalendarSurfaceEntry,
  CalendarViewMode,
} from "@/services/calendar/contracts";
import { addDays, toDayKey } from "@/services/calendar/date";

const props = defineProps<{
  viewMode: CalendarViewMode;
  anchorDate: Date;
  visibleDays: Date[];
  rangeLabel: string;
}>();

const emit = defineEmits<{
  "update:viewMode": [viewMode: CalendarViewMode];
  previous: [];
  next: [];
  today: [];
}>();

const { wrapClass, wrapStyle } = useSidebarState();
const store = useTodoStore();
const { todos, projects } = storeToRefs(store);

const projectTitles = computed(() => {
  const map = new Map<string, string>();
  for (const project of projects.value) {
    map.set(project.id, project.title);
  }
  return map;
});

const surfaceEntries = computed<CalendarSurfaceEntry[]>(() => {
  const todayKey = toDayKey(new Date());

  return todos.value
    .filter((todo) => !todo.isCompleted && !todo.isCancelled && !todo.isTrashed)
    .filter((todo) => todo.scheduledDate || todo.isToday)
    .map((todo) => {
      const dayKey = todo.scheduledDate ?? todayKey;
      return {
        id: `task:${todo.id}`,
        title: todo.title,
        subtitle: todo.projectId ? projectTitles.value.get(todo.projectId) ?? null : null,
        link: null,
        color: "#2563eb",
        start: dayKey,
        end: toDayKey(addDays(new Date(`${dayKey}T00:00:00`), 1)),
        isAllDay: true,
      };
    })
    .toSorted((left, right) => left.title.localeCompare(right.title, "ru"));
});

const connectionMessage = "Показываются только локальные задачи Delphi.";
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
    <div :class="[wrapClass, 'flex min-h-0 flex-1 flex-col gap-4 px-4 pb-6']" :style="wrapStyle">
      <div
        class="flex flex-wrap items-center gap-3 rounded-2xl border border-(--border) bg-(--background) px-4 py-3"
      >
        <div class="flex items-center gap-2">
          <button
            type="button"
            class="rounded-xl border border-(--border) p-2 text-(--muted-foreground) transition-colors hover:text-(--foreground)"
            @click="emit('previous')"
          >
            <ChevronLeft :size="16" />
          </button>
          <button
            type="button"
            class="rounded-xl border border-(--border) px-3 py-2 text-sm font-medium text-(--foreground) transition-colors hover:bg-(--secondary)"
            @click="emit('today')"
          >
            Сегодня
          </button>
          <button
            type="button"
            class="rounded-xl border border-(--border) p-2 text-(--muted-foreground) transition-colors hover:text-(--foreground)"
            @click="emit('next')"
          >
            <ChevronRight :size="16" />
          </button>
        </div>

        <div class="min-w-0 flex-1">
          <div class="truncate text-sm font-semibold text-(--foreground)">
            {{ rangeLabel }}
          </div>
          <div class="truncate text-xs text-(--muted-foreground)">
            {{ connectionMessage }}
          </div>
        </div>

        <CalendarViewSwitch
          :model-value="viewMode"
          @update:model-value="emit('update:viewMode', $event)"
        />
      </div>

      <component
        :is="viewMode === 'month' ? CalendarMonthGrid : CalendarTimeGrid"
        :anchor-date="anchorDate"
        :days="visibleDays"
        :entries="surfaceEntries"
      />
    </div>
  </div>
</template>
