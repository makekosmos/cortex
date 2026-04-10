<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { Plus } from "lucide-vue-next";
import { useQuickEntry } from "@/composables/useQuickEntry";
import type { TodoItem } from "@/types/task";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";

const { show: openQuickEntry } = useQuickEntry();
const store = useTodoStore();
const { todos, projects } = storeToRefs(store);

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

function startOfDay(date: Date): Date {
  const d = new Date(date);
  d.setHours(0, 0, 0, 0);
  return d;
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

function toISODate(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function getWeekDays(): Date[] {
  const today = startOfDay(new Date());
  const dow = today.getDay();
  const mondayOffset = dow === 0 ? -6 : 1 - dow;
  const monday = new Date(today);
  monday.setDate(today.getDate() + mondayOffset);
  return Array.from({ length: 7 }, (_, i) => {
    const d = new Date(monday);
    d.setDate(monday.getDate() + i);
    return d;
  });
}

const weekDays = computed(() => getWeekDays());
const todayDate = computed(() => startOfDay(new Date()));

const dayNameFmt = new Intl.DateTimeFormat("ru-RU", { weekday: "short" });
const dayMonthFmt = new Intl.DateTimeFormat("ru-RU", {
  day: "numeric",
  month: "short",
});
const monthYearFmt = new Intl.DateTimeFormat("ru-RU", {
  month: "long",
  year: "numeric",
});

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

// ---------------------------------------------------------------------------
// Week label
// ---------------------------------------------------------------------------

const weekLabel = computed(() => {
  const days = weekDays.value;
  if (days[0].getMonth() === days[6].getMonth()) {
    return capitalize(monthYearFmt.format(days[0]));
  }
  return `${dayMonthFmt.format(days[0])} – ${dayMonthFmt.format(days[6])}`;
});

// ---------------------------------------------------------------------------
// Project lookup
// ---------------------------------------------------------------------------

const projectById = computed(() => {
  const map = new Map<string, string>();
  for (const p of projects.value) map.set(p.id, p.title);
  return map;
});

// ---------------------------------------------------------------------------
// Columns
// ---------------------------------------------------------------------------

type DayColumn = {
  date: Date;
  dateStr: string;
  isToday: boolean;
  isWeekend: boolean;
  dayName: string;
  dayNumber: string;
  todos: TodoItem[];
};

function colTodosFor(date: Date, dateStr: string, all: TodoItem[]): TodoItem[] {
  const isToday = isSameDay(date, todayDate.value);
  return all
    .filter((t) => {
      if (t.scheduledDate && t.scheduledDate.startsWith(dateStr)) return true;
      if (isToday && t.isToday && !t.scheduledDate) return true;
      return false;
    })
    .sort((a, b) => {
      if (a.isEvening !== b.isEvening) return a.isEvening ? 1 : -1;
      return a.sortOrder - b.sortOrder;
    });
}

const activeTodos = computed(() =>
  todos.value.filter((t) => !t.isCompleted && !t.isCancelled && !t.isTrashed),
);

const columns = computed<DayColumn[]>(() =>
  weekDays.value.map((date) => {
    const dow = date.getDay();
    const dateStr = toISODate(date);
    return {
      date,
      dateStr,
      isToday: isSameDay(date, todayDate.value),
      isWeekend: dow === 0 || dow === 6,
      dayName: capitalize(dayNameFmt.format(date)),
      dayNumber: String(date.getDate()),
      todos: colTodosFor(date, dateStr, activeTodos.value),
    };
  }),
);

// ---------------------------------------------------------------------------
// Drag & Drop
// ---------------------------------------------------------------------------

const draggingId = shallowRef<string | null>(null);
const overColDate = shallowRef<string | null>(null);
const overCardId = shallowRef<string | null>(null);
const dropAfterCard = shallowRef(false);

function onCardDragStart(todoId: string, e: DragEvent) {
  draggingId.value = todoId;
  e.dataTransfer!.effectAllowed = "move";
  e.dataTransfer!.setData("text/plain", todoId);
  // Defer opacity so the ghost captures the full card
  requestAnimationFrame(() => {
    draggingId.value = todoId;
  });
}

function onCardDragEnd() {
  draggingId.value = null;
  overColDate.value = null;
  overCardId.value = null;
}

function onColDragOver(colDateStr: string, e: DragEvent) {
  e.preventDefault();
  e.dataTransfer!.dropEffect = "move";
  overColDate.value = colDateStr;

  const cardEl = (e.target as HTMLElement).closest<HTMLElement>("[data-card-id]");
  if (cardEl && cardEl.dataset.cardId !== draggingId.value) {
    const rect = cardEl.getBoundingClientRect();
    overCardId.value = cardEl.dataset.cardId!;
    dropAfterCard.value = e.clientY > rect.top + rect.height / 2;
  } else if (!cardEl) {
    overCardId.value = null;
  }
}

function onColDragLeave(colDateStr: string, e: DragEvent) {
  const rel = e.relatedTarget as HTMLElement | null;
  const col = e.currentTarget as HTMLElement;
  if (rel && col.contains(rel)) return;
  if (overColDate.value === colDateStr) {
    overColDate.value = null;
    overCardId.value = null;
  }
}

function onColDrop(col: DayColumn, e: DragEvent) {
  e.preventDefault();
  if (!draggingId.value) return;

  const todoId = draggingId.value;
  const todo = todos.value.find((t) => t.id === todoId);
  if (!todo) {
    onCardDragEnd();
    return;
  }

  // Build ordered list for the target column (excluding dragged item)
  const colItems = colTodosFor(col.date, col.dateStr, activeTodos.value);
  const others = colItems.filter((t) => t.id !== todoId);

  // Find insert position
  let insertIdx = others.length; // default: end
  if (overCardId.value) {
    const idx = others.findIndex((t) => t.id === overCardId.value);
    if (idx >= 0) insertIdx = dropAfterCard.value ? idx + 1 : idx;
  }

  // Re-apply sort orders
  const reordered = [...others];
  reordered.splice(insertIdx, 0, todo);
  reordered.forEach((t, i) => {
    if (t.sortOrder !== i) store.updateTodo(t.id, { sortOrder: i });
  });

  // Update scheduledDate / isToday
  const updates: Partial<TodoItem> = {};
  const alreadyInCol =
    (todo.scheduledDate && todo.scheduledDate.startsWith(col.dateStr)) ||
    (col.isToday && todo.isToday && !todo.scheduledDate);

  if (!alreadyInCol) {
    updates.scheduledDate = col.dateStr;
    if (!col.isToday && todo.isToday) updates.isToday = false;
  }

  if (Object.keys(updates).length > 0) store.updateTodo(todoId, updates);

  onCardDragEnd();
}
</script>

<template>
  <div class="flex min-h-0 w-full min-w-0 flex-1 flex-col">
    <!-- Header -->
    <div class="flex shrink-0 items-center gap-2.5 px-7 pb-3 pt-6">
<h1 class="text-2xl font-bold text-(--foreground) select-none">Неделя</h1>
      <span class="text-sm text-(--muted-foreground)/70 select-none">
        {{ weekLabel }}
      </span>
    </div>

    <!-- Kanban board -->
    <div class="flex-1 overflow-x-auto overflow-y-hidden">
      <div class="flex h-full gap-2.5 px-4 pb-6 pt-1" style="min-width: max-content">
        <div
          v-for="col in columns"
          :key="col.dateStr"
          class="flex h-full w-52 shrink-0 flex-col rounded-xl transition-colors"
          :class="[
            overColDate === col.dateStr
              ? 'ring-2 ring-blue-500/60 bg-(--surface)'
              : col.isToday
                ? 'bg-(--surface) ring-1 ring-(--border)'
                : col.isWeekend
                  ? 'bg-(--secondary)/30'
                  : 'bg-(--secondary)/50',
          ]"
          @dragover="onColDragOver(col.dateStr, $event)"
          @dragleave="onColDragLeave(col.dateStr, $event)"
          @drop="onColDrop(col, $event)"
        >
          <!-- Column header -->
          <div class="flex shrink-0 items-center gap-2 px-3 pb-2.5 pt-3">
            <div
              class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-sm font-bold select-none"
              :class="
                col.isToday
                  ? 'bg-(--primary) text-(--primary-foreground)'
                  : 'text-(--muted-foreground)'
              "
            >
              {{ col.dayNumber }}
            </div>
            <div class="flex min-w-0 flex-1 flex-col">
              <span
                class="text-xs font-semibold uppercase tracking-wider select-none"
                :class="
                  col.isToday
                    ? 'text-(--foreground)'
                    : col.isWeekend
                      ? 'text-(--muted-foreground)/60'
                      : 'text-(--muted-foreground)'
                "
              >
                {{ col.dayName }}
              </span>
            </div>
            <span
              v-if="col.todos.length > 0"
              class="shrink-0 text-xs text-(--muted-foreground)/50 select-none"
            >
              {{ col.todos.length }}
            </span>
          </div>

          <!-- Divider -->
          <div class="mx-3 mb-1.5 shrink-0 border-t border-(--border)/60" />

          <!-- Task list -->
          <div class="flex flex-1 flex-col gap-1.5 overflow-y-auto px-2 pb-2">
            <div
              v-if="col.todos.length === 0 && overColDate !== col.dateStr"
              class="flex flex-1 items-center justify-center"
            >
              <span class="text-xs text-(--muted-foreground)/25 select-none">
                Нет задач
              </span>
            </div>

            <template v-for="todo in col.todos" :key="todo.id">
              <!-- Drop indicator: before card -->
              <div
                v-if="
                  overColDate === col.dateStr &&
                  overCardId === todo.id &&
                  !dropAfterCard
                "
                class="mx-1 h-0.5 rounded-full bg-blue-500/70"
              />

              <!-- Task card -->
              <div
                :data-card-id="todo.id"
                draggable="true"
                class="group rounded-lg bg-(--background) p-2.5 transition-all"
                :class="[
                  draggingId === todo.id
                    ? 'opacity-40 scale-95'
                    : 'cursor-grab hover:shadow-sm active:cursor-grabbing',
                ]"
                @dragstart="onCardDragStart(todo.id, $event)"
                @dragend="onCardDragEnd"
              >
                <div class="flex items-start gap-2">
                  <!-- Checkbox -->
                  <button
                    type="button"
                    class="mt-0.5 shrink-0"
                    @pointerdown.stop
                    @click.stop="store.completeTodo(todo.id)"
                  >
                    <div
                      class="h-[15px] w-[15px] rounded-full border border-(--ring) transition-colors hover:bg-(--ring)"
                    />
                  </button>

                  <!-- Content -->
                  <div class="min-w-0 flex-1">
                    <div class="break-words text-xs leading-snug text-(--foreground) select-none">
                      {{ todo.title }}
                    </div>
                    <div
                      v-if="todo.projectId && projectById.get(todo.projectId)"
                      class="mt-0.5 truncate text-[10px] text-(--muted-foreground)/60 select-none"
                    >
                      {{ projectById.get(todo.projectId) }}
                    </div>
                    <div
                      v-if="todo.isEvening"
                      class="mt-0.5 text-[10px] text-indigo-400/80 select-none"
                    >
                      Вечер
                    </div>
                  </div>
                </div>

                <!-- Quick actions -->
                <div
                  class="mt-1.5 flex gap-1 opacity-0 transition-opacity group-hover:opacity-100"
                >
                  <button
                    type="button"
                    class="rounded px-1 py-0.5 text-[9px] text-(--muted-foreground) transition-colors hover:bg-red-500/15 hover:text-red-500"
                    @pointerdown.stop
                    @click.stop="store.trashTodo(todo.id)"
                  >
                    <span class="select-none">Удалить</span>
                  </button>
                </div>
              </div>

              <!-- Drop indicator: after card -->
              <div
                v-if="
                  overColDate === col.dateStr &&
                  overCardId === todo.id &&
                  dropAfterCard
                "
                class="mx-1 h-0.5 rounded-full bg-blue-500/70"
              />
            </template>

            <!-- Drop indicator at end when column is empty or no card hovered -->
            <div
              v-if="overColDate === col.dateStr && !overCardId"
              class="mx-1 h-0.5 rounded-full bg-blue-500/70"
            />
          </div>
        </div>
      </div>
    </div>

    <!-- FAB -->
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
