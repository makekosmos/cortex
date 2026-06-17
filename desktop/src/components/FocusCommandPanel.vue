<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from "vue";
import { Info, X } from "@lucide/vue";
import { Dropdown } from "@kosmos/visuals";
import type { FocusBlockedApp, FocusSessionSnapshot, FocusSessionTask } from "@shared/ipc-types";
import { buildFocusSessionStartInput } from "./focusCommandPayload";

const durationOptions = [25, 45, 60, 90];
const doneTaskStatuses = new Set(["done", "canceled", "cancelled"]);

interface FocusAppEntry {
  id: string;
  name: string;
  exec_path?: string;
  icon_path?: string | null;
  icon_ref?: string | null;
}

const snapshot = shallowRef<FocusSessionSnapshot | null>(null);
const tasks = shallowRef<FocusSessionTask[]>([]);
const apps = shallowRef<FocusAppEntry[]>([]);
const title = ref("");
const durationMin = ref(25);
const durationMode = ref("25");
const customDurationText = ref("");
const taskId = ref<string | null>(null);
const focusMode = ref<"block" | "allow">("block");
const titleFocused = ref(false);
const highlightedTaskIndex = ref(0);
const appPickerValue = ref<string | null>(null);
const blockedApps = shallowRef<FocusAppEntry[]>([]);
const submitting = ref(false);
const error = ref("");
let unsubscribeUpdated: (() => void) | null = null;

const goalInputRef = ref<HTMLDivElement | null>(null);
let suppressGoalSync = false;

function createChipEl(task: FocusSessionTask): HTMLSpanElement {
  const chip = document.createElement("span");
  chip.dataset.chip = task.id;
  chip.contentEditable = "false";
  chip.style.cssText = [
    "display:inline-flex",
    "align-items:center",
    "gap:3px",
    "vertical-align:middle",
    "border:1px solid color-mix(in srgb,var(--accent) 38%,transparent)",
    "border-radius:5px",
    "background:color-mix(in srgb,var(--accent) 16%,transparent)",
    "color:color-mix(in srgb,var(--accent) 90%,var(--foreground))",
    "padding:1px 5px 1px 6px",
    "font-size:11px",
    "font-weight:700",
    "line-height:1.5",
    "margin:0 2px",
    "user-select:none",
    "-webkit-user-select:none",
  ].join(";");

  const label = document.createElement("span");
  label.textContent = task.title;
  label.style.cssText = "max-width:180px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap";
  chip.appendChild(label);

  const btn = document.createElement("button");
  btn.type = "button";
  btn.setAttribute("aria-label", "Убрать задачу");
  btn.style.cssText = [
    "display:inline-flex",
    "align-items:center",
    "justify-content:center",
    "width:12px",
    "height:12px",
    "border:none",
    "background:transparent",
    "color:currentColor",
    "padding:0",
    "opacity:0.65",
    "cursor:default",
    "font-size:14px",
    "line-height:1",
  ].join(";");
  btn.textContent = "×";
  btn.addEventListener("mousedown", (e) => {
    e.preventDefault();
    removeTask();
  });
  chip.appendChild(btn);
  return chip;
}

function syncGoalDOM(): void {
  const el = goalInputRef.value;
  if (!el) return;
  suppressGoalSync = true;
  const task = selectedTask.value;
  const text = title.value;
  el.innerHTML = "";
  if (task) {
    el.appendChild(createChipEl(task));
    el.appendChild(document.createTextNode(" " + text));
  } else {
    el.appendChild(document.createTextNode(text));
  }
  // курсор в конец
  const range = document.createRange();
  range.selectNodeContents(el);
  range.collapse(false);
  const sel = window.getSelection();
  sel?.removeAllRanges();
  sel?.addRange(range);
  suppressGoalSync = false;
}

function onGoalInput(): void {
  if (suppressGoalSync) return;
  const el = goalInputRef.value;
  if (!el) return;
  let text = "";
  el.childNodes.forEach((node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      text += (node.textContent ?? "").replace(/^ /, "");
    }
  });
  title.value = text;
  onTitleInput();
}

const selectedTask = computed(() => tasks.value.find((task) => task.id === taskId.value) ?? null);
const durationDropdownOptions = computed(() => [
  ...durationOptions.map((minutes) => ({
    value: String(minutes),
    label: `${minutes} минут`,
  })),
  { value: "custom", label: "Свое время" },
]);
const visibleTasks = computed(() =>
  tasks.value.filter((task) => !task.status || !doneTaskStatuses.has(task.status)),
);
const mentionState = computed(() => {
  const at = title.value.lastIndexOf("@");
  if (at < 0) return null;
  const tail = title.value.slice(at + 1);
  if (/\s/.test(tail)) return null;
  return { at, query: tail };
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
const blockedAppIds = computed(() => blockedApps.value.map((app) => app.id));
const blockedAppPayload = computed<FocusBlockedApp[]>(() =>
  blockedApps.value.map((app) => ({
    id: app.id,
    name: app.name,
    icon: app.icon_ref ?? app.icon_path ?? null,
    exec_path: app.exec_path ?? null,
  })),
);
const appDropdownOptions = computed(() =>
  apps.value
    .filter((app) => !blockedAppIds.value.includes(app.id))
    .map((app) => ({
      value: app.id,
      label: app.name,
    })),
);
const resolvedDurationMin = computed(() => {
  if (durationMode.value === "custom") {
    const parsed = Number.parseInt(customDurationText.value, 10);
    return Number.isFinite(parsed) ? Math.max(1, Math.min(24 * 60, parsed)) : 25;
  }
  return Number.parseInt(durationMode.value, 10);
});

async function hydrate(): Promise<void> {
  error.value = "";
  try {
    const [nextSnapshot, nextTasks, nextApps] = await Promise.all([
      window.kepler.focusSession.snapshot(),
      window.kepler.focusSession.listTasks(),
      fetchApps(),
    ]);
    snapshot.value = nextSnapshot;
    tasks.value = nextTasks;
    apps.value = nextApps;
    syncFormFromSnapshot(nextSnapshot);
  } catch (e) {
    error.value = `Не удалось загрузить фокус: ${String((e as Error)?.message ?? e)}`;
  }
}

async function fetchApps(): Promise<FocusAppEntry[]> {
  try {
    const res = await window.kepler.ark.request<{ apps?: FocusAppEntry[] }>("app_index.list_all", {
      limit: 500,
    });
    return res.apps ?? [];
  } catch (e) {
    console.warn("app_index.list_all failed", e);
    return [];
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
  const ids = new Set(nextSnapshot.focus.blocked_app_ids ?? []);
  if (ids.size > 0) {
    blockedApps.value = apps.value.filter((app) => ids.has(app.id));
  } else {
    blockedApps.value = [];
  }
  void nextTick(syncGoalDOM);
}

function onTitleInput(): void {
  highlightedTaskIndex.value = 0;
}

function pickTask(task: FocusSessionTask): void {
  const state = mentionState.value;
  const prefix = state ? title.value.slice(0, state.at).trimEnd() : title.value.trimEnd();
  const suffix = state ? title.value.slice(state.at + state.query.length + 1).trimStart() : "";
  taskId.value = task.id;
  title.value = [prefix, suffix].filter(Boolean).join(" ");
  titleFocused.value = false;
  void nextTick(syncGoalDOM);
}

function removeTask(): void {
  taskId.value = null;
  void nextTick(syncGoalDOM);
}

function pickBlockedApp(app: FocusAppEntry): void {
  if (!blockedApps.value.some((item) => item.id === app.id)) {
    blockedApps.value = [...blockedApps.value, app];
  }
}

function removeBlockedApp(appId: string): void {
  blockedApps.value = blockedApps.value.filter((app) => app.id !== appId);
}

function appById(id: string | number | null | undefined): FocusAppEntry | null {
  if (typeof id !== "string") return null;
  return apps.value.find((app) => app.id === id) ?? null;
}

function appIcon(app: FocusAppEntry | null | undefined): string | null {
  return app?.icon_ref ?? app?.icon_path ?? null;
}

function onTitleKeydown(event: KeyboardEvent): void {
  if (event.key === "Enter") {
    event.preventDefault(); // не добавлять <br> в contenteditable
    if (mentionOpen.value) {
      const task = mentionTasks.value[highlightedTaskIndex.value];
      if (task) pickTask(task);
    }
    return;
  }
  if (event.key === "Backspace" && !title.value && selectedTask.value) {
    event.preventDefault();
    removeTask();
    return;
  }
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
        categoryIds: [],
        blockedAppIds: blockedAppIds.value,
        blockedApps: blockedAppPayload.value,
      }),
    );
    await window.kepler.window.hide();
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

watch(appPickerValue, (id) => {
  if (!id) return;
  const app = apps.value.find((item) => item.id === id);
  if (app) pickBlockedApp(app);
  appPickerValue.value = null;
});

onUnmounted(() => {
  unsubscribeUpdated?.();
});

defineExpose({ start });
</script>

<template>
  <section class="focus-command" aria-label="Фокус-сессия">
    <!-- См. postmortems.md § 2026-06-05: Shell command pages render defaults immediately. -->
    <div class="focus-command__layout">
      <form class="focus-command__form" @submit.prevent="start">
        <div class="focus-command__body">
          <label class="focus-command__field">
            <span class="focus-command__label">Цель</span>
            <div class="focus-command__goal-wrap">
              <div
                ref="goalInputRef"
                class="focus-command__goal-box"
                :class="{
                  'focus-command__goal-box--focused': titleFocused,
                  'focus-command__goal-box--empty': !title && !taskId,
                }"
                contenteditable="true"
                spellcheck="false"
                data-placeholder="Что сейчас делаешь  @задача"
                @input="onGoalInput"
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
          </label>
          <div class="focus-command__field">
            <span class="focus-command__label">Длительность</span>
            <div class="focus-command__duration-row">
              <Dropdown
                v-model="durationMode"
                :options="durationDropdownOptions"
                :searchable="false"
                :max-height-px="160"
                class="focus-command__duration-dropdown"
              />
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
          <div class="focus-command__field">
            <span class="focus-command__label">Режим</span>
            <div class="focus-command__mode" role="group" aria-label="Режим фокуса">
              <button
                type="button"
                class="focus-command__mode-button"
                :class="{ 'focus-command__mode-button--active': focusMode === 'block' }"
                @click="focusMode = 'block'"
              >
                Блокировать
              </button>
              <button
                type="button"
                class="focus-command__mode-button"
                disabled
                title="Allow mode пока недоступен"
              >
                Разрешать
              </button>
            </div>
          </div>
          <div class="focus-command__field">
            <span class="focus-command__label">Блокировка</span>
            <div class="focus-command__block-control">
              <div v-if="blockedApps.length > 0" class="focus-command__app-chips">
                <button
                  v-for="app in blockedApps"
                  :key="app.id"
                  type="button"
                  class="focus-command__app-chip"
                  @click="removeBlockedApp(app.id)"
                >
                  <img
                    v-if="appIcon(app)"
                    :src="appIcon(app)!"
                    class="focus-command__chip-icon"
                    alt=""
                  />
                  <span v-else class="focus-command__chip-fallback">{{ app.name.charAt(0) }}</span>
                  <span>{{ app.name }}</span>
                  <X :size="12" />
                </button>
              </div>
              <Dropdown
                v-model="appPickerValue"
                :options="appDropdownOptions"
                :searchable="true"
                :max-height-px="160"
                placeholder="Добавить приложение…"
                search-placeholder="Поиск приложения…"
                class="focus-command__app-dropdown"
              >
                <template #trigger-leading="{ option }">
                  <img
                    v-if="appIcon(appById(option?.value))"
                    :src="appIcon(appById(option?.value))!"
                    class="focus-command__dropdown-icon"
                    alt=""
                  />
                </template>
                <template #option-leading="{ option }">
                  <img
                    v-if="appIcon(appById(option.value))"
                    :src="appIcon(appById(option.value))!"
                    class="focus-command__dropdown-icon"
                    alt=""
                  />
                  <span v-else class="focus-command__dropdown-fallback">{{
                    option.label.charAt(0)
                  }}</span>
                </template>
              </Dropdown>
              <Info
                class="focus-command__block-info"
                :size="18"
                aria-label="Блокируются приложения из лаунчера Kosmos"
              />
            </div>
          </div>
        </div>

        <div v-if="error" class="focus-command__error-inline">{{ error }}</div>
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

.focus-command__layout {
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.focus-command__form {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.focus-command__body {
  display: grid;
  align-content: start;
  gap: 14px;
  width: min(760px, calc(100% - 48px));
  margin: 0 auto;
  padding: 20px 0 16px;
  min-width: 0;
  flex: 1;
  overflow-y: auto;
}

.focus-command__field {
  display: grid;
  grid-template-columns: 100px minmax(0, 1fr);
  align-items: center;
  gap: 16px;
  min-width: 0;
}

.focus-command__label {
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 12px;
  font-weight: 650;
  line-height: 32px;
  text-align: right;
}

.focus-command__field input {
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

.focus-command__field input:focus {
  border-color: color-mix(in srgb, var(--accent) 58%, transparent);
}

.focus-command__goal-wrap {
  position: relative;
  min-width: 0;
}

.focus-command__goal-box {
  min-height: 32px;
  min-width: 0;
  border: 1px solid color-mix(in srgb, var(--foreground) 13%, transparent);
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  padding: 5px 10px;
  font-size: 13px;
  color: var(--foreground);
  line-height: 1.6;
  outline: none;
  word-break: break-word;
  -webkit-user-select: text;
  user-select: text;
  transition: border-color 140ms;
}

.focus-command__goal-box--focused {
  border-color: color-mix(in srgb, var(--accent) 58%, transparent);
}

.focus-command__goal-box--empty::before {
  content: attr(data-placeholder);
  color: color-mix(in srgb, var(--foreground) 36%, transparent);
  pointer-events: none;
}

.focus-command__mention {
  position: absolute;
  z-index: 5;
  top: calc(100% + 6px);
  left: 0;
  display: grid;
  width: min(100%, 360px);
  max-height: 160px;
  overflow-y: auto;
  padding: 4px;
  border: 1px solid color-mix(in srgb, var(--foreground) 13%, transparent);
  border-radius: 6px;
  background: var(--popover, var(--background));
  box-shadow: var(--shadow-floating);
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
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.focus-command__duration-dropdown {
  min-width: 0;
  flex: 1 1 auto;
}

.focus-command__duration-row input {
  max-width: 104px;
}

.focus-command__mode {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.focus-command__mode-button {
  height: 28px;
  border: 0;
  border-radius: 14px;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 58%, transparent);
  padding: 0 12px;
  font: inherit;
  font-size: 13px;
  font-weight: 700;
}

.focus-command__mode-button--active {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
  color: var(--foreground);
}

.focus-command__mode-button:disabled {
  opacity: 0.6;
}

.focus-command__block-control {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 40px;
  min-width: 0;
  border: 1px solid color-mix(in srgb, var(--foreground) 13%, transparent);
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  padding: 5px 8px;
}

.focus-command__app-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  flex: 0 1 auto;
  min-width: 0;
}

.focus-command__app-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 100%;
  height: 26px;
  border: 1px solid color-mix(in srgb, var(--foreground) 12%, transparent);
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 12%, transparent);
  color: var(--foreground);
  padding: 0 8px;
  font: inherit;
  font-size: 12px;
  font-weight: 700;
}

.focus-command__chip-icon,
.focus-command__dropdown-icon {
  flex: 0 0 auto;
  width: 16px;
  height: 16px;
  border-radius: 4px;
  object-fit: cover;
}

.focus-command__chip-fallback,
.focus-command__dropdown-fallback {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--accent) 42%, var(--background));
  color: var(--foreground);
  font-size: 10px;
  font-weight: 800;
  text-transform: uppercase;
}

.focus-command__app-chip span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.focus-command__app-dropdown {
  min-width: 190px;
  flex: 1 1 auto;
}

.focus-command__block-info {
  flex: 0 0 auto;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
}

.focus-command__error-inline {
  padding: 0 24px 8px;
  color: var(--danger, color-mix(in srgb, var(--foreground) 80%, red));
  font-size: 12px;
}
</style>
