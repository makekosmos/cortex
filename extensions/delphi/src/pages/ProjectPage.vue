<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Circle, DollarSign, MoreHorizontal, Plus } from "@lucide/vue";
import { useQuickEntry } from "@/composables/useQuickEntry";
import { useSidebarState } from "@/composables/useSidebarState";

const { show: openQuickEntry } = useQuickEntry();
const {
  wrapClass,
  wrapStyle,
  titleWrapRef,
  titleGroupRef,
  titleGroupClass,
  titleGroupStyle,
  titleClass,
} = useSidebarState();
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { ProjectStatus } from "@/types/task";
import { TodoRow } from "@kosmos/visuals";

// ---------------------------------------------------------------------------
// Color tag helper
// ---------------------------------------------------------------------------

function colorTagClass(colorTag?: string | null): string {
  switch (colorTag) {
    case "red":
      return "text-red-500";
    case "orange":
      return "text-orange-500";
    case "yellow":
      return "text-yellow-500";
    case "green":
      return "text-green-500";
    case "blue":
      return "text-blue-500";
    case "purple":
      return "text-purple-500";
    case "pink":
      return "text-pink-500";
    default:
      return "text-(--muted-foreground)";
  }
}

// ---------------------------------------------------------------------------
// Router & store
// ---------------------------------------------------------------------------

const router = useRouter();
const route = useRoute();
const store = useTodoStore();
const { projects } = storeToRefs(store);

const id = computed(() => route.params.id as string | undefined);

const project = computed(() => projects.value.find((p) => p.id === id.value));

const todos = computed(() => (id.value ? store.todosForProject(id.value) : []));

const activeTodos = computed(() => todos.value.filter((t) => !t.isCompleted && !t.isCancelled));

const completedTodos = computed(() => todos.value.filter((t) => t.isCompleted || t.isCancelled));

// ---------------------------------------------------------------------------
// Time entries — read from ARK (Horologion / Strontium) and distribute price
// ---------------------------------------------------------------------------

interface TimeEntrySummary {
  taskId: string | null;
  billable: boolean;
  startedAt: string;
  endedAt: string | null;
}

const timeEntries = ref<TimeEntrySummary[]>([]);
let timeEntryPollHandle: ReturnType<typeof setInterval> | null = null;
const isElectronEnv = typeof window !== "undefined" && Boolean(window.electronAPI);

async function refreshTimeEntries() {
  if (!isElectronEnv) return;
  try {
    const result = (await window.electronAPI?.invoke("ark:listTimeEntries")) as
      | TimeEntrySummary[]
      | undefined;
    timeEntries.value = result ?? [];
  } catch {
    timeEntries.value = [];
  }
}

onMounted(() => {
  void refreshTimeEntries();
  timeEntryPollHandle = setInterval(refreshTimeEntries, 15000);
});

onUnmounted(() => {
  if (timeEntryPollHandle) clearInterval(timeEntryPollHandle);
});

function entryDurationSec(entry: TimeEntrySummary): number {
  const start = new Date(entry.startedAt).getTime();
  if (!Number.isFinite(start)) return 0;
  const end = entry.endedAt ? new Date(entry.endedAt).getTime() : Date.now();
  return Math.max(0, Math.floor((end - start) / 1000));
}

const billableSecondsByTask = computed(() => {
  const out = new Map<string, number>();
  for (const entry of timeEntries.value) {
    if (!entry.billable || !entry.taskId) continue;
    out.set(entry.taskId, (out.get(entry.taskId) ?? 0) + entryDurationSec(entry));
  }
  return out;
});

const totalBillableSeconds = computed(() => {
  let total = 0;
  for (const t of todos.value) {
    total += billableSecondsByTask.value.get(t.id) ?? 0;
  }
  return total;
});

const totalBillableHours = computed(() => totalBillableSeconds.value / 3600);

const projectHourlyRate = computed<number | null>(() => {
  const price = project.value?.price ?? null;
  if (price === null || totalBillableHours.value <= 0) return null;
  return price / totalBillableHours.value;
});

function formatHours(hours: number): string {
  return hours.toFixed(2).replace(/\.?0+$/, "") + " ч";
}

function formatPrice(value: number): string {
  return value.toFixed(value % 1 === 0 ? 0 : 2);
}

// ---------------------------------------------------------------------------
// Editable title
// ---------------------------------------------------------------------------

const editing = shallowRef(false);
const editTitle = shallowRef("");
const inputRef = ref<HTMLInputElement | null>(null);

watch(editing, async (val) => {
  if (val) {
    await nextTick();
    inputRef.value?.focus();
    inputRef.value?.select();
  }
});

function startRename() {
  if (!project.value) return;
  editTitle.value = project.value.title;
  editing.value = true;
  menuOpen.value = false;
}

function commitRename() {
  editing.value = false;
  const trimmed = editTitle.value.trim();
  if (project.value && trimmed && trimmed !== project.value.title) {
    store.updateProject(project.value.id, { title: trimmed });
  }
}

// ---------------------------------------------------------------------------
// Context menu
// ---------------------------------------------------------------------------

const menuOpen = shallowRef(false);
const menuRef = ref<HTMLDivElement | null>(null);

function handleOutsideClick(e: MouseEvent) {
  if (menuRef.value && !menuRef.value.contains(e.target as Node)) {
    menuOpen.value = false;
  }
}

watch(menuOpen, (val) => {
  if (val) {
    document.addEventListener("mousedown", handleOutsideClick);
  } else {
    document.removeEventListener("mousedown", handleOutsideClick);
  }
});

onUnmounted(() => {
  document.removeEventListener("mousedown", handleOutsideClick);
});

function handleDrop(payload: { targetId: string; after: boolean }, sourceId: string) {
  if (sourceId === payload.targetId) return;
  const list = [...activeTodos.value];
  const srcIdx = list.findIndex((t) => t.id === sourceId);
  if (srcIdx === -1) return;
  const [item] = list.splice(srcIdx, 1);
  const tgtIdx = list.findIndex((t) => t.id === payload.targetId);
  if (tgtIdx === -1) return;
  list.splice(payload.after ? tgtIdx + 1 : tgtIdx, 0, item);
  list.forEach((t, i) => store.updateTodo(t.id, { sortOrder: i }));
}

function handleDelete() {
  if (!project.value) return;
  menuOpen.value = false;
  store.removeProject(project.value.id);
  router.push("/");
}

function handleArchive() {
  if (!project.value) return;
  menuOpen.value = false;
  store.updateProject(project.value.id, { status: ProjectStatus.Completed });
  router.push("/");
}
</script>

<template>
  <!-- Project not found -->
  <div v-if="!project" class="flex w-full min-w-0 flex-col items-center justify-center">
    <p class="text-(--muted-foreground)">Проект не найден</p>
  </div>

  <!-- Project view -->
  <div v-else class="flex min-h-0 w-full min-w-0 flex-1 flex-col">
    <!-- Header -->
    <div
      ref="titleWrapRef"
      :class="[wrapClass, 'flex min-h-8 items-center gap-2.5 px-7 pb-3 pt-6']"
      :style="wrapStyle"
    >
      <Circle :size="12" :class="['shrink-0 fill-current', colorTagClass(project.colorTag)]" />

      <div ref="titleGroupRef" :class="titleGroupClass" :style="titleGroupStyle">
        <input
          v-if="editing"
          ref="inputRef"
          v-model="editTitle"
          :class="[
            titleClass,
            'bg-transparent text-2xl font-bold text-(--foreground) outline-none',
          ]"
          type="text"
          @blur="commitRename"
          @keydown.enter="commitRename"
          @keydown.escape="editing = false"
        />
        <h1
          v-else
          :class="[titleClass, 'text-2xl font-bold text-(--foreground) select-none cursor-pointer']"
          @dblclick="startRename"
        >
          {{ project.title }}
        </h1>

        <span v-if="todos.length > 0" class="text-sm text-(--muted-foreground)">
          {{ activeTodos.length }}
        </span>

        <span
          v-if="project.billable"
          class="ml-2 flex items-center gap-1 rounded-full bg-emerald-500/15 px-2.5 py-0.5 text-xs text-emerald-500"
        >
          <DollarSign :size="11" />
          <span v-if="project.price !== null && project.price !== undefined">
            {{ formatPrice(project.price) }}
          </span>
          <span v-else>оплачиваемый</span>
        </span>

        <span
          v-if="totalBillableSeconds > 0"
          class="ml-1 rounded-full bg-(--secondary) px-2.5 py-0.5 text-xs text-(--muted-foreground)"
          :title="
            projectHourlyRate !== null
              ? `${formatPrice(projectHourlyRate)} / час`
              : 'Сумма оплачиваемого времени по задачам проекта'
          "
        >
          {{ formatHours(totalBillableHours) }}
          <template v-if="projectHourlyRate !== null">
            · {{ formatPrice(projectHourlyRate) }}/ч
          </template>
        </span>
      </div>

      <!-- Context menu -->
      <div ref="menuRef" class="absolute right-7">
        <button
          type="button"
          class="rounded p-1 text-(--muted-foreground) hover:bg-(--secondary) hover:text-(--foreground)"
          @click="menuOpen = !menuOpen"
        >
          <MoreHorizontal :size="18" />
        </button>

        <div
          v-if="menuOpen"
          class="absolute right-0 top-full z-50 mt-1 w-44 rounded-lg border border-(--border) bg-(--popover) py-1 shadow-lg"
        >
          <button
            type="button"
            class="flex w-full items-center px-3 py-2 text-sm text-(--foreground) hover:bg-(--secondary)"
            @click="startRename"
          >
            Переименовать
          </button>
          <button
            type="button"
            class="flex w-full items-center px-3 py-2 text-sm text-(--foreground) hover:bg-(--secondary)"
            @click="handleArchive"
          >
            Архивировать
          </button>
          <div class="my-1 border-t border-(--border)" />
          <button
            type="button"
            class="flex w-full items-center px-3 py-2 text-sm text-red-500 hover:bg-red-500/10"
            @click="handleDelete"
          >
            Удалить
          </button>
        </div>
      </div>
    </div>

    <!-- Task list -->
    <div class="scrollbar-gutter flex-1 overflow-y-auto">
      <div :class="[wrapClass, 'pb-20 pt-1']" :style="wrapStyle">
        <div
          v-if="activeTodos.length === 0 && completedTodos.length === 0"
          class="px-7 py-10 text-center text-sm text-(--muted-foreground)/60"
        >
          Нет задач в проекте
        </div>
        <template v-else>
          <div v-if="activeTodos.length > 0" class="flex flex-col">
            <TodoRow
              v-for="todo in activeTodos"
              :key="todo.id"
              :todo="todo"
              @complete="store.completeTodo(todo.id)"
              @trash="store.trashTodo(todo.id)"
              @update="store.updateTodo(todo.id, $event)"
              @drop="handleDrop($event, todo.id)"
            >
              <span
                v-if="(billableSecondsByTask.get(todo.id) ?? 0) > 0"
                class="rounded-full bg-(--secondary) px-2 py-0.5 text-[10px] text-(--muted-foreground)"
                :title="'Оплачиваемое время по задаче'"
              >
                {{ formatHours((billableSecondsByTask.get(todo.id) ?? 0) / 3600) }}
              </span>
            </TodoRow>
          </div>

          <template v-if="completedTodos.length > 0">
            <div
              class="px-7 pb-1 pt-4 text-xs font-semibold uppercase tracking-wider text-(--muted-foreground)/60 select-none"
            >
              Завершённые ({{ completedTodos.length }})
            </div>
            <div class="flex flex-col">
              <TodoRow
                v-for="todo in completedTodos"
                :key="todo.id"
                :todo="todo"
                @complete="store.completeTodo(todo.id)"
              />
            </div>
          </template>
        </template>
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
