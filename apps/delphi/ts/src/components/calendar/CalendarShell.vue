<script setup lang="ts">
import { computed, onMounted, shallowRef, watch } from "vue";
import { RouterLink } from "vue-router";
import { ChevronLeft, ChevronRight, RefreshCw } from "lucide-vue-next";
import { storeToRefs } from "pinia";
import CalendarMonthGrid from "@/components/calendar/CalendarMonthGrid.vue";
import CalendarTimeGrid from "@/components/calendar/CalendarTimeGrid.vue";
import { useSidebarState } from "@/composables/useSidebarState";
import { useTodoStore } from "@/store/todos";
import { useGoogleCalendar } from "@/composables/useGoogleCalendar";
import type {
  CalendarSurfaceEntry,
  GoogleCalendarViewMode,
} from "@/services/google-calendar/contracts";
import { addDays, toDayKey } from "@/services/google-calendar/date";

const props = defineProps<{
  viewMode: GoogleCalendarViewMode;
  anchorDate: Date;
  visibleDays: Date[];
  visibleRange: {
    start: Date;
    end: Date;
    request: {
      start: string;
      end: string;
    };
  };
  rangeLabel: string;
}>();

const emit = defineEmits<{
  previous: [];
  next: [];
  today: [];
}>();

const { wrapClass, wrapStyle } = useSidebarState();
const store = useTodoStore();
const { todos, projects } = storeToRefs(store);
const googleCalendar = useGoogleCalendar();
const initialLoadDone = shallowRef(false);

const projectTitles = computed(() => {
  const map = new Map<string, string>();
  for (const project of projects.value) {
    map.set(project.id, project.title);
  }
  return map;
});

const taskEntries = computed<CalendarSurfaceEntry[]>(() => {
  const todayKey = toDayKey(new Date());

  return todos.value
    .filter((todo) => !todo.isCompleted && !todo.isCancelled && !todo.isTrashed)
    .filter((todo) => todo.scheduledDate || todo.isToday)
    .map((todo) => {
      const dayKey = todo.scheduledDate ?? todayKey;
      return {
        id: `task:${todo.id}`,
        source: "task" as const,
        title: todo.title,
        subtitle: todo.projectId ? projectTitles.value.get(todo.projectId) ?? null : null,
        link: null,
        color: "#2563eb",
        start: dayKey,
        end: toDayKey(addDays(new Date(`${dayKey}T00:00:00`), 1)),
        isAllDay: true,
      };
    });
});

const googleEntries = computed<CalendarSurfaceEntry[]>(() =>
  googleCalendar.events.value.map((event) => ({
    id: `google:${event.calendarId}:${event.id}`,
    source: "google" as const,
    title: event.title,
    subtitle: event.calendarSummary || event.location,
    link: event.htmlLink,
    color: event.color,
    start: event.start,
    end: event.end,
    isAllDay: event.isAllDay,
  })),
);

const surfaceEntries = computed(() =>
  [...taskEntries.value, ...googleEntries.value].toSorted((left, right) =>
    left.title.localeCompare(right.title, "ru"),
  ),
);

const connectionMessage = computed(() => {
  if (!googleCalendar.isSupported()) {
    return "Google Calendar доступен только в Electron-сборке.";
  }

  if (!googleCalendar.status.value.configured) {
    return "Google OAuth не настроен в этой сборке.";
  }

  if (!googleCalendar.status.value.connected) {
    return "Google Calendar не подключен. Войдите через OAuth в настройках.";
  }

  if (googleCalendar.status.value.lastSyncError) {
    return googleCalendar.status.value.lastSyncError;
  }

  if (googleCalendar.status.value.lastSyncAt) {
    return `Последняя синхронизация: ${new Date(
      googleCalendar.status.value.lastSyncAt,
    ).toLocaleString("ru-RU")}`;
  }

  return "Календарь подключен.";
});

async function refreshForVisibleRange() {
  if (!googleCalendar.status.value.connected) return;
  await googleCalendar.refresh(props.visibleRange.request, true);
}

async function syncVisibleRangeIfNeeded() {
  if (!googleCalendar.status.value.connected) return;
  await googleCalendar.refresh(props.visibleRange.request);
}

onMounted(async () => {
  await googleCalendar.load();
  initialLoadDone.value = true;
  if (googleCalendar.status.value.connected) {
    await syncVisibleRangeIfNeeded();
  }
});

watch(
  () => [
    props.visibleRange.request.start,
    props.visibleRange.request.end,
    googleCalendar.status.value.connected,
  ],
  async ([start, end, connected], [prevStart, prevEnd, prevConnected]) => {
    if (!initialLoadDone.value || !connected) return;
    if (
      start === prevStart &&
      end === prevEnd &&
      connected === prevConnected
    ) {
      return;
    }
    await syncVisibleRangeIfNeeded();
  },
);
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

        <div class="flex items-center gap-2">
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-xl border border-(--border) px-3 py-2 text-sm font-medium text-(--foreground) transition-colors hover:bg-(--secondary)"
            :disabled="googleCalendar.loading.value"
            @click="refreshForVisibleRange"
          >
            <RefreshCw
              :size="15"
              :class="googleCalendar.loading.value ? 'animate-spin' : ''"
            />
            Обновить
          </button>

          <RouterLink
            to="/settings"
            class="rounded-xl border border-(--border) px-3 py-2 text-sm font-medium text-(--foreground) transition-colors hover:bg-(--secondary)"
          >
            Настройки
          </RouterLink>
        </div>
      </div>

      <div
        v-if="!googleCalendar.status.value.configured"
        class="rounded-2xl border border-amber-500/30 bg-amber-500/8 px-5 py-4 text-sm text-(--foreground)"
      >
        В этой сборке Google OAuth не настроен. Когда конфиг встроен, здесь останется только вход через Google.
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
