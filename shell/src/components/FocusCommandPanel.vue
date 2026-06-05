<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef } from "vue";
import { Play, Shield, Target } from "@lucide/vue";
import type { FocusBlocklist, FocusSessionSnapshot, FocusSessionTask } from "@shared/ipc-types";
import { buildFocusSessionStartInput } from "./focusCommandPayload";

const durationOptions = [25, 45, 60, 90];
const doneTaskStatuses = new Set(["done", "canceled", "cancelled"]);

const snapshot = shallowRef<FocusSessionSnapshot | null>(null);
const tasks = shallowRef<FocusSessionTask[]>([]);
const blocklists = shallowRef<FocusBlocklist[]>([]);
const title = ref("");
const durationMin = ref(25);
const durationMode = ref("25");
const customDurationText = ref("");
const taskId = ref<string | null>(null);
const titleFocused = ref(false);
const highlightedTaskIndex = ref(0);
const selectedBlocklistIds = ref<string[]>([]);
const loading = ref(true);
const submitting = ref(false);
const error = ref("");
let unsubscribeUpdated: (() => void) | null = null;

const active = computed(() => snapshot.value !== null && snapshot.value.pomodoro.phase !== "idle");
const selectedTask = computed(() => tasks.value.find((task) => task.id === taskId.value) ?? null);
const visibleTasks = computed(() =>
  tasks.value.filter((task) => !task.status || !doneTaskStatuses.has(task.status)),
);
const mentionState = computed(() => {
  const at = title.value.lastIndexOf("@");
  if (at < 0) return null;
  const query = title.value.slice(at + 1).trimStart();
  return { at, query };
});
const mentionTasks = computed(() => {
  const q = mentionState.value?.query.toLowerCase().trim() ?? "";
  const source = visibleTasks.value;
  if (!q) return source.slice(0, 8);
  return source.filter((task) => task.title.toLowerCase().includes(q)).slice(0, 8);
});
const mentionOpen = computed(
  () => titleFocused.value && mentionState.value !== null && mentionTasks.value.length > 0,
);
const resolvedDurationMin = computed(() => {
  if (durationMode.value === "custom") {
    const parsed = Number.parseInt(customDurationText.value, 10);
    return Number.isFinite(parsed) ? Math.max(1, Math.min(24 * 60, parsed)) : 25;
  }
  return Number.parseInt(durationMode.value, 10);
});

async function hydrate(): Promise<void> {
  loading.value = true;
  error.value = "";
  try {
    const [nextSnapshot, nextTasks, nextBlocklists] = await Promise.all([
      window.kepler.focusSession.snapshot(),
      window.kepler.focusSession.listTasks(),
      window.kepler.focusSession.listBlocklists(),
    ]);
    snapshot.value = nextSnapshot;
    tasks.value = nextTasks;
    blocklists.value = nextBlocklists;
    syncFormFromSnapshot(nextSnapshot);
  } catch (e) {
    error.value = `Не удалось загрузить фокус: ${String((e as Error)?.message ?? e)}`;
  } finally {
    loading.value = false;
  }
}

function syncFormFromSnapshot(nextSnapshot: FocusSessionSnapshot): void {
  const pomodoro = nextSnapshot.pomodoro;
  title.value = pomodoro.title?.trim() || "";
  durationMin.value = Math.max(1, Math.round((pomodoro.totalMs || 25 * 60_000) / 60_000));
  if (durationOptions.includes(durationMin.value)) {
    durationMode.value = String(durationMin.value);
    customDurationText.value = "";
  } else {
    durationMode.value = "custom";
    customDurationText.value = String(durationMin.value);
  }
  taskId.value = pomodoro.tasks?.[0]?.id ?? null;
  selectedBlocklistIds.value = nextSnapshot.focus.blocklist_id
    ? [nextSnapshot.focus.blocklist_id]
    : [];
}

function onTitleInput(): void {
  if (selectedTask.value && !title.value.includes(selectedTask.value.title)) {
    taskId.value = null;
  }
  highlightedTaskIndex.value = 0;
}

function pickTask(task: FocusSessionTask): void {
  const state = mentionState.value;
  const prefix = state ? title.value.slice(0, state.at).trimEnd() : title.value.trim();
  taskId.value = task.id;
  title.value = prefix ? `${prefix} ${task.title}` : task.title;
  titleFocused.value = false;
}

function onTitleKeydown(event: KeyboardEvent): void {
  if (!mentionOpen.value) return;
  if (event.key === "ArrowDown") {
    event.preventDefault();
    highlightedTaskIndex.value = Math.min(
      highlightedTaskIndex.value + 1,
      mentionTasks.value.length - 1,
    );
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    highlightedTaskIndex.value = Math.max(highlightedTaskIndex.value - 1, 0);
  } else if (event.key === "Enter") {
    const task = mentionTasks.value[highlightedTaskIndex.value];
    if (task) {
      event.preventDefault();
      pickTask(task);
    }
  } else if (event.key === "Escape") {
    titleFocused.value = false;
  }
}

function toggleBlocklist(id: string): void {
  const next = new Set(selectedBlocklistIds.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  selectedBlocklistIds.value = Array.from(next);
}

async function start(): Promise<void> {
  submitting.value = true;
  error.value = "";
  try {
    snapshot.value = await window.kepler.focusSession.start(
      buildFocusSessionStartInput({
        title: title.value,
        durationMin: resolvedDurationMin.value,
        taskId: selectedTask.value?.id ?? null,
        taskTitle: selectedTask.value?.title ?? null,
        categoryIds: selectedBlocklistIds.value,
      }),
    );
  } catch (e) {
    error.value = `Не удалось начать фокус: ${String((e as Error)?.message ?? e)}`;
  } finally {
    submitting.value = false;
  }
}

onMounted(() => {
  void hydrate();
  unsubscribeUpdated = window.kepler.focusSession.onUpdated(() => {
    void hydrate();
  });
});

onUnmounted(() => {
  unsubscribeUpdated?.();
});
</script>

<template>
  <section class="focus-command" aria-label="Фокус-сессия">
    <div v-if="loading" class="focus-command__state">Загружаю фокус</div>
    <div v-else class="focus-command__layout">
      <form class="focus-command__form" @submit.prevent="start">
        <div class="focus-command__section">
          <div class="focus-command__section-title">
            <Target :size="15" />
            <span>Сессия</span>
          </div>
          <label class="focus-command__field">
            <span>Цель</span>
            <div class="focus-command__title-wrap">
              <input
                v-model="title"
                type="text"
                placeholder="Что сейчас делаешь  @задача"
                autocomplete="off"
                spellcheck="false"
                @input="onTitleInput"
                @focus="titleFocused = true"
                @blur="titleFocused = false"
                @keydown="onTitleKeydown"
              />
              <div
                v-if="mentionOpen"
                class="focus-command__mention"
                role="listbox"
                aria-label="Задачи Delphi"
              >
                <button
                  v-for="(task, index) in mentionTasks"
                  :key="task.id"
                  type="button"
                  role="option"
                  class="focus-command__mention-item"
                  :class="{ 'focus-command__mention-item--active': index === highlightedTaskIndex }"
                  :aria-selected="index === highlightedTaskIndex"
                  @mouseenter="highlightedTaskIndex = index"
                  @mousedown.prevent="pickTask(task)"
                >
                  <span>{{ task.title }}</span>
                  <small v-if="task.status">{{ task.status }}</small>
                </button>
              </div>
            </div>
            <small v-if="selectedTask" class="focus-command__task-chip">
              @{{ selectedTask.title }}
            </small>
          </label>
          <div class="focus-command__field">
            <span>Длительность</span>
            <div class="focus-command__duration-row">
              <select v-model="durationMode">
                <option v-for="minutes in durationOptions" :key="minutes" :value="String(minutes)">
                  {{ minutes }} минут
                </option>
                <option value="custom">Свое время</option>
              </select>
              <input
                v-if="durationMode === 'custom'"
                v-model="customDurationText"
                type="number"
                min="1"
                max="1440"
                inputmode="numeric"
                aria-label="Своя длительность в минутах"
                placeholder="Минут"
              />
            </div>
          </div>
        </div>

        <div class="focus-command__section focus-command__section--blocks">
          <div class="focus-command__section-title">
            <Shield :size="15" />
            <span>Блокировки</span>
          </div>
          <div v-if="blocklists.length === 0" class="focus-command__empty">
            Списки блокировок пока не настроены
          </div>
          <div v-else class="focus-command__blocklists">
            <button
              v-for="blocklist in blocklists"
              :key="blocklist.id"
              type="button"
              class="focus-command__blocklist"
              :class="{
                'focus-command__blocklist--active': selectedBlocklistIds.includes(blocklist.id),
              }"
              @click="toggleBlocklist(blocklist.id)"
            >
              <span class="focus-command__blocklist-check" aria-hidden="true" />
              <span class="focus-command__blocklist-main">
                <span>{{ blocklist.name }}</span>
                <small>{{ blocklist.domains.length }} доменов</small>
              </span>
            </button>
          </div>
        </div>

        <div class="focus-command__footer">
          <div v-if="error" class="focus-command__error">{{ error }}</div>
          <button class="focus-command__primary" type="submit" :disabled="submitting">
            <Play :size="15" />
            <span>{{ active ? "Перезапустить фокус" : "Начать фокус" }}</span>
          </button>
        </div>
      </form>
    </div>
  </section>
</template>

<style scoped>
.focus-command {
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.focus-command__state,
.focus-command__empty {
  display: grid;
  place-items: center;
  color: color-mix(in srgb, var(--foreground) 48%, transparent);
  font-size: 13px;
}

.focus-command__layout {
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.focus-command__form {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.focus-command__section {
  display: grid;
  gap: 10px;
  padding: 12px 16px;
}

.focus-command__section--blocks {
  align-content: start;
  min-height: 0;
  overflow-y: auto;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
}

.focus-command__section-title,
.focus-command__primary,
.focus-command__duration-row,
.focus-command__blocklist {
  display: flex;
  align-items: center;
}

.focus-command__section-title,
.focus-command__field {
  display: grid;
  gap: 5px;
  min-width: 0;
}

.focus-command__field > span {
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 12px;
  font-weight: 650;
}

.focus-command__field input,
.focus-command__field select {
  width: 100%;
  min-width: 0;
  height: 32px;
  border: 1px solid color-mix(in srgb, var(--foreground) 13%, transparent);
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  color: var(--foreground);
  padding: 0 10px;
  font: inherit;
  font-size: 13px;
  outline: none;
}

.focus-command__field input:focus,
.focus-command__field select:focus {
  border-color: color-mix(in srgb, var(--accent) 58%, transparent);
}

.focus-command__primary,
.focus-command__blocklist {
  border: 1px solid color-mix(in srgb, var(--foreground) 12%, transparent);
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  color: var(--foreground);
  font: inherit;
}

.focus-command__title-wrap {
  position: relative;
  min-width: 0;
}

.focus-command__task-chip {
  overflow: hidden;
  color: color-mix(in srgb, var(--accent) 82%, var(--foreground));
  font-size: 11px;
  font-weight: 700;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.focus-command__mention {
  position: absolute;
  z-index: 5;
  top: calc(100% + 6px);
  left: 0;
  display: grid;
  width: min(100%, 360px);
  max-height: 190px;
  overflow-y: auto;
  padding: 4px;
  border: 1px solid color-mix(in srgb, var(--foreground) 13%, transparent);
  border-radius: 6px;
  background: var(--popover, var(--background));
  box-shadow: 0 14px 34px color-mix(in srgb, var(--foreground) 20%, transparent);
}

.focus-command__mention-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-width: 0;
  height: 32px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--foreground);
  padding: 0 8px;
  text-align: left;
  font: inherit;
  font-size: 12px;
}

.focus-command__mention-item--active,
.focus-command__mention-item:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.focus-command__mention-item span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.focus-command__mention-item small {
  flex: 0 0 auto;
  color: color-mix(in srgb, var(--foreground) 48%, transparent);
  font-size: 10px;
  text-transform: uppercase;
}

.focus-command__duration-row {
  gap: 8px;
}

.focus-command__duration-row input {
  max-width: 104px;
}

.focus-command__primary {
  border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  background: color-mix(in srgb, var(--accent) 15%, transparent);
}

.focus-command__blocklists {
  display: grid;
  gap: 7px;
}

.focus-command__blocklist {
  gap: 10px;
  width: 100%;
  min-height: 44px;
  padding: 8px 10px;
  text-align: left;
}

.focus-command__blocklist--active {
  border-color: color-mix(in srgb, var(--accent) 48%, transparent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}

.focus-command__blocklist-check {
  display: block;
  width: 12px;
  height: 12px;
  flex: 0 0 auto;
  border: 1px solid color-mix(in srgb, var(--foreground) 32%, transparent);
  border-radius: 3px;
}

.focus-command__blocklist--active .focus-command__blocklist-check {
  border-color: var(--accent);
  background: var(--accent);
}

.focus-command__blocklist-main {
  display: grid;
  min-width: 0;
  gap: 3px;
}

.focus-command__blocklist-main span,
.focus-command__blocklist-main small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.focus-command__blocklist-main span {
  font-size: 13px;
  font-weight: 700;
}

.focus-command__blocklist-main small {
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
  font-size: 11px;
}

.focus-command__footer {
  display: grid;
  gap: 8px;
  padding: 10px 16px 12px;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
}

.focus-command__error {
  color: var(--danger, var(--foreground));
  font-size: 12px;
}

.focus-command__primary {
  justify-content: center;
  gap: 8px;
  height: 38px;
  font-size: 13px;
  font-weight: 750;
}

.focus-command__primary:disabled {
  cursor: default;
  opacity: 0.55;
}
</style>
