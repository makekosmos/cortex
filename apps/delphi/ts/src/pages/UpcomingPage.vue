<script setup lang="ts">
import { computed } from "vue";
import { Calendar, Plus } from "lucide-vue-next";
import { useQuickEntry } from "@/composables/useQuickEntry";

const { show: openQuickEntry } = useQuickEntry();
import type { TodoItem } from "@/types/task";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";

// ---------------------------------------------------------------------------
// Section type
// ---------------------------------------------------------------------------

type UpcomingSection = {
  id: string;
  title: string;
  dateLabel: string;
  todos: TodoItem[];
};

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

function startOfDay(date: Date): Date {
  const d = new Date(date);
  d.setHours(0, 0, 0, 0);
  return d;
}

function addDays(date: Date, days: number): Date {
  const d = new Date(date);
  d.setDate(d.getDate() + days);
  return d;
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

const shortDateFmt = new Intl.DateTimeFormat("ru-RU", {
  weekday: "short",
  month: "short",
  day: "numeric",
});

const dayNameFmt = new Intl.DateTimeFormat("ru-RU", { weekday: "long" });

const monthFmt = new Intl.DateTimeFormat("ru-RU", {
  month: "long",
  year: "numeric",
});

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function dayTitle(date: Date, offset: number): string {
  if (offset === 1) return "Завтра";
  return capitalize(dayNameFmt.format(date));
}

function weekOfYear(date: Date): number {
  const d = new Date(
    Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()),
  );
  const dayNum = d.getUTCDay() || 7;
  d.setUTCDate(d.getUTCDate() + 4 - dayNum);
  const yearStart = new Date(Date.UTC(d.getUTCFullYear(), 0, 1));
  return Math.ceil(((d.getTime() - yearStart.getTime()) / 86400000 + 1) / 7);
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

const store = useTodoStore();
const { todos, projects } = storeToRefs(store);

// Upcoming todos: has scheduledDate, not completed/cancelled/trashed/someday
const upcomingTodos = computed(() =>
  todos.value.filter(
    (t) =>
      t.scheduledDate &&
      !t.isCompleted &&
      !t.isCancelled &&
      !t.isTrashed &&
      !t.isSomeday,
  ),
);

const sections = computed<UpcomingSection[]>(() => {
  const today = startOfDay(new Date());
  const result: UpcomingSection[] = [];

  // Next 7 days individually
  for (let offset = 1; offset <= 7; offset++) {
    const date = addDays(today, offset);
    const dayTodos = upcomingTodos.value
      .filter((t) => {
        if (!t.scheduledDate) return false;
        return isSameDay(new Date(t.scheduledDate), date);
      })
      .toSorted((a, b) => a.sortOrder - b.sortOrder);

    result.push({
      id: `day-${offset}`,
      title: dayTitle(date, offset),
      dateLabel: shortDateFmt.format(date),
      todos: dayTodos,
    });
  }

  // Weeks 2-4 beyond the first 7 days
  const weekBoundary = addDays(today, 8);
  for (let weekOffset = 2; weekOffset <= 4; weekOffset++) {
    const weekStart = addDays(weekBoundary, (weekOffset - 2) * 7);
    const weekEnd = addDays(weekStart, 7);

    const weekTodos = upcomingTodos.value
      .filter((t) => {
        if (!t.scheduledDate) return false;
        const d = new Date(t.scheduledDate);
        return d >= weekStart && d < weekEnd;
      })
      .toSorted((a, b) => {
        const da = a.scheduledDate ?? "";
        const db = b.scheduledDate ?? "";
        return da.localeCompare(db);
      });

    if (weekTodos.length > 0) {
      const wn = weekOfYear(weekStart);
      result.push({
        id: `week-${wn}`,
        title: `Неделя ${wn}`,
        dateLabel: shortDateFmt.format(weekStart),
        todos: weekTodos,
      });
    }
  }

  // Months beyond that, up to 1 year
  const monthBoundary = addDays(today, 30);
  const yearBoundary = addDays(today, 365);
  let cursor = new Date(
    monthBoundary.getFullYear(),
    monthBoundary.getMonth(),
    1,
  );

  while (cursor < yearBoundary) {
    const nextMonth = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 1);
    const monthTodos = upcomingTodos.value
      .filter((t) => {
        if (!t.scheduledDate) return false;
        const d = new Date(t.scheduledDate);
        return d >= cursor && d < nextMonth;
      })
      .toSorted((a, b) => {
        const da = a.scheduledDate ?? "";
        const db = b.scheduledDate ?? "";
        return da.localeCompare(db);
      });

    if (monthTodos.length > 0) {
      const label = capitalize(monthFmt.format(cursor));
      result.push({
        id: `month-${label}`,
        title: label,
        dateLabel: "",
        todos: monthTodos,
      });
    }
    cursor = nextMonth;
  }

  return result;
});

const projectById = computed(() => {
  const map = new Map<string, string>();
  for (const p of projects.value) {
    map.set(p.id, p.title);
  }
  return map;
});

function handleToggleToday(todo: TodoItem) {
  store.updateTodo(todo.id, { isToday: !todo.isToday });
}
</script>

<template>
  <div class="flex min-h-0 w-full min-w-0 flex-1 flex-col">
    <!-- Header -->
    <div
      class="mx-auto w-full max-w-(--bringhurst-wide) flex items-center justify-center gap-2.5 px-7 pb-3 pt-6"
    >
      <Calendar :size="24" class="text-red-500" />
      <h1 class="text-2xl font-bold text-(--foreground) select-none">Планы</h1>
    </div>

    <!-- Scrollable content -->
    <div class="scrollbar-gutter flex-1 overflow-y-auto">
      <div class="mx-auto w-full max-w-(--bringhurst-wide) pb-20 pt-1">
        <div v-for="section in sections" :key="section.id">
          <!-- Section header -->
          <div
            class="sticky top-0 z-10 flex items-center gap-2 bg-(--background) px-7 py-2"
          >
            <span class="text-sm font-bold text-(--foreground)">
              {{ section.title }}
            </span>
            <span
              v-if="section.todos.length > 0"
              class="text-xs font-medium text-(--muted-foreground)"
            >
              {{ section.todos.length }}
            </span>
            <div class="flex-1" />
            <span
              v-if="section.dateLabel"
              class="text-xs text-(--muted-foreground)/60"
            >
              {{ section.dateLabel }}
            </span>
          </div>

          <!-- Section content -->
          <div
            v-if="section.todos.length === 0"
            class="px-7 py-2 text-xs text-(--muted-foreground)/50"
          >
            Нет задач
          </div>
          <div v-else class="flex flex-col">
            <div
              v-for="todo in section.todos"
              :key="todo.id"
              class="group flex items-center gap-3 px-7 py-2 hover:bg-(--secondary)"
              @contextmenu.prevent
            >
              <!-- Checkbox -->
              <button
                type="button"
                class="shrink-0"
                @click="store.completeTodo(todo.id)"
              >
                <div
                  class="h-[18px] w-[18px] rounded-full border border-(--ring) transition-colors hover:bg-(--ring)"
                />
              </button>

              <!-- Content -->
              <div class="min-w-0 flex-1">
                <div class="truncate text-sm text-(--foreground)">
                  {{ todo.title }}
                </div>
                <div
                  v-if="todo.projectId && projectById.get(todo.projectId)"
                  class="truncate text-xs text-(--muted-foreground)"
                >
                  {{ projectById.get(todo.projectId) }}
                </div>
              </div>

              <!-- Quick actions (visible on hover) -->
              <div
                class="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100"
              >
                <button
                  type="button"
                  title="На сегодня"
                  :class="[
                    'rounded px-1.5 py-0.5 text-[10px] transition-colors',
                    todo.isToday
                      ? 'bg-yellow-500/15 text-yellow-500'
                      : 'text-(--muted-foreground) hover:bg-(--accent)',
                  ]"
                  @click="handleToggleToday(todo)"
                >
                  Сегодня
                </button>
                <button
                  type="button"
                  title="В корзину"
                  class="rounded px-1.5 py-0.5 text-[10px] text-(--muted-foreground) hover:bg-red-500/15 hover:text-red-500"
                  @click="store.trashTodo(todo.id)"
                >
                  Удалить
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <button
      type="button"
      class="absolute bottom-6 right-6 flex h-12 w-12 items-center justify-center rounded-full bg-(--primary) text-(--primary-foreground) shadow-lg transition-transform hover:scale-105 active:scale-95"
      title="Новая задача (⌘N)"
      @click="openQuickEntry"
    >
      <Plus :size="24" />
    </button>
  </div>
</template>
