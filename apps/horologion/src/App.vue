<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, computed, nextTick, watch } from "vue";
import { RouterView, RouterLink, useRoute } from "vue-router";
import { Play, Pause, Settings } from "lucide-vue-next";
import { DesktopChrome, DesktopContentSurface } from "@kosmos/visuals";
import type {
  TimeEntry,
  ArkStatus,
  ArkConnectionStatus,
  DelphiTask,
} from "@shared/ipc-types";
import { formatDuration } from "./lib/format";
import {
  notifyEntriesChanged,
  currentDraft,
  entriesChangedAt,
  tasks as tasksRef,
  loadTasksOnce,
  ensureFreshTasks,
} from "./lib/store";
import MentionMenu from "./components/MentionMenu.vue";

const route = useRoute();

const draft = ref("");
const running = ref<TimeEntry | null>(null);
const elapsed = ref(0);
const errMsg = ref<string | null>(null);
let tickHandle: ReturnType<typeof setInterval> | null = null;

// --- ARK status ---
const arkStatus = ref<ArkConnectionStatus>("connecting");
const arkStatusMessage = ref<string>("Подключение к ARK…");
const arkDbPath = ref<string>("");
let arkPollHandle: ReturnType<typeof setInterval> | null = null;

async function refreshArkStatus() {
  try {
    const s: ArkStatus = await window.horologion.ark.status();
    arkStatus.value = s.status;
    arkDbPath.value = s.dbPath ?? "";
    arkStatusMessage.value =
      s.status === "connected"
        ? `ARK подключен${s.dbPath ? ` · ${s.dbPath}` : ""}`
        : s.status === "connecting"
          ? "Подключение к ARK…"
          : `Ошибка ARK: ${s.message ?? "unknown"}`;
  } catch (e) {
    arkStatus.value = "error";
    arkStatusMessage.value = `Не удалось получить статус: ${e instanceof Error ? e.message : e}`;
  }
}

// Класс точки соответствует тону подключения: success / warning / danger.
const arkDotClass = computed(() => `dot dot--${arkStatus.value}`);

// --- @-mention ---
const inputRef = ref<HTMLInputElement | null>(null);
const contentRef = ref<HTMLElement | null>(null);

// JS-driven fade для скроллбара. CSS transition на webkit-scrollbar-thumb
// в Chromium не пересчитывается на toggle класса — поэтому анимируем CSS-var
// `--sb-alpha` через rAF, и webkit-scrollbar-thumb читает его в background-color.
// Логика: scroll → alpha=1 мгновенно. Idle 500ms → 300ms fade alpha=1→0.
let scrollIdleTimer: ReturnType<typeof setTimeout> | null = null;
let scrollFadeRaf: number | null = null;

function setSbAlpha(a: number) {
  const el = contentRef.value;
  if (!el) return;
  el.style.setProperty("--sb-alpha", a.toFixed(3));
}

function onContentScroll() {
  if (scrollIdleTimer) clearTimeout(scrollIdleTimer);
  if (scrollFadeRaf !== null) {
    cancelAnimationFrame(scrollFadeRaf);
    scrollFadeRaf = null;
  }
  setSbAlpha(1);
  scrollIdleTimer = setTimeout(() => {
    const start = performance.now();
    const FADE_MS = 300;
    const step = (now: number) => {
      const elapsed = now - start;
      const p = Math.min(1, elapsed / FADE_MS);
      // ease-out cubic
      const eased = 1 - Math.pow(1 - p, 3);
      setSbAlpha(1 - eased);
      if (p < 1) {
        scrollFadeRaf = requestAnimationFrame(step);
      } else {
        scrollFadeRaf = null;
      }
    };
    scrollFadeRaf = requestAnimationFrame(step);
  }, 500);
}
const mentionOpen = ref(false);
const mentionQuery = ref("");
const mentionAnchor = ref(0);
const mentionHighlight = ref(0);
const selectedTaskId = ref<string | null>(null);
const selectedTaskTitle = ref<string | null>(null);

// Tasks shared через store: грузим один раз, переиспользуем в Pomodoro / EditModal.
const tasks = tasksRef;
const mentionMenuRef = ref<InstanceType<typeof MentionMenu> | null>(null);

async function loadTasks() {
  await ensureFreshTasks();
}

function onInput() {
  const el = inputRef.value;
  if (!el) return;
  const caret = el.selectionStart ?? draft.value.length;
  let anchor = -1;
  for (let i = caret - 1; i >= 0; i--) {
    const ch = draft.value[i];
    if (ch === "@") {
      const prev = i === 0 ? " " : draft.value[i - 1];
      if (/\s/.test(prev) || i === 0) anchor = i;
      break;
    }
    if (/\s/.test(ch)) break;
  }
  if (anchor >= 0) {
    const query = draft.value.slice(anchor + 1, caret);
    if (/^[\p{L}\p{N}_\- ]*$/u.test(query)) {
      mentionAnchor.value = anchor;
      mentionQuery.value = query;
      mentionOpen.value = true;
      mentionHighlight.value = 0;
      void loadTasks();
      return;
    }
  }
  mentionOpen.value = false;
}

function pickTask(task: DelphiTask) {
  const before = draft.value.slice(0, mentionAnchor.value);
  const caret = inputRef.value?.selectionStart ?? draft.value.length;
  const after = draft.value.slice(caret);
  const inserted = `@${task.title} `;
  draft.value = before + inserted + after;
  selectedTaskId.value = task.id;
  selectedTaskTitle.value = task.title;
  mentionOpen.value = false;
  nextTick(() => {
    const el = inputRef.value;
    if (!el) return;
    const pos = (before + inserted).length;
    el.focus();
    el.setSelectionRange(pos, pos);
  });
}

function onKeyDown(e: KeyboardEvent) {
  if (mentionOpen.value) {
    const filtered = mentionMenuRef.value?.filtered ?? [];
    if (e.key === "ArrowDown") {
      e.preventDefault();
      mentionHighlight.value = Math.min(mentionHighlight.value + 1, filtered.length - 1);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      mentionHighlight.value = Math.max(0, mentionHighlight.value - 1);
      return;
    }
    if (e.key === "Enter") {
      const pick = filtered[mentionHighlight.value];
      if (pick) {
        e.preventDefault();
        pickTask(pick);
        return;
      }
    }
    if (e.key === "Escape") {
      e.preventDefault();
      mentionOpen.value = false;
      return;
    }
  }
  if (e.key === "Enter" && !mentionOpen.value) {
    e.preventDefault();
    void toggleTimer();
  }
}

// --- timer ---

async function refreshRunning() {
  const list = await window.horologion.timeEntries.listRunning();
  running.value = list[0] ?? null;
  if (running.value) {
    if (draft.value === "") draft.value = running.value.title;
    startTick();
  } else {
    stopTick();
    elapsed.value = 0;
  }
}

function startTick() {
  if (tickHandle) return;
  tickHandle = setInterval(() => {
    if (!running.value) return;
    const startedAt = new Date(running.value.startedAt).getTime();
    elapsed.value = Math.floor((Date.now() - startedAt) / 1000);
  }, 1000);
}

function stopTick() {
  if (tickHandle) {
    clearInterval(tickHandle);
    tickHandle = null;
  }
}

async function toggleTimer() {
  errMsg.value = null;
  mentionOpen.value = false;
  try {
    if (running.value) {
      await window.horologion.timeEntries.stopTimer(running.value.id);
      running.value = null;
      stopTick();
      elapsed.value = 0;
      draft.value = "";
      selectedTaskId.value = null;
      selectedTaskTitle.value = null;
      notifyEntriesChanged();
    } else {
      const title = draft.value.trim() || "Без названия";
      const entry = await window.horologion.timeEntries.startTimer({
        title,
        taskId: selectedTaskId.value,
        taskTitle: selectedTaskTitle.value,
      });
      running.value = entry;
      startTick();
      notifyEntriesChanged();
    }
  } catch (e) {
    errMsg.value = e instanceof Error ? e.message : String(e);
    console.error("toggleTimer failed:", e);
  }
}

const timerLabel = computed(() =>
  running.value ? formatDuration(elapsed.value) : "0:00:00",
);

onMounted(() => {
  void refreshRunning();
  void loadTasks();
  void refreshArkStatus();
  arkPollHandle = setInterval(refreshArkStatus, 2000);
});

// Любое изменение в entries (включая создание/закрытие записей из Pomodoro)
// триггерит refresh running-state в top-bar.
watch(entriesChangedAt, () => {
  void refreshRunning();
});

// Синхронизируем currentDraft с UI top-bar'а, чтобы PomodoroView знал что юзер
// печатает / какая задача выбрана.
watch(
  [draft, selectedTaskId, selectedTaskTitle] as const,
  () => {
    currentDraft.value = {
      title: draft.value,
      taskId: selectedTaskId.value,
      taskTitle: selectedTaskTitle.value,
    };
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  if (arkPollHandle) clearInterval(arkPollHandle);
});

const tabs = [
  { name: "list", label: "Список", path: "/list" },
  { name: "pomodoro", label: "Помодоро", path: "/pomodoro" },
] as const;

function isTab(t: string): boolean {
  return route.name === t;
}
</script>

<template>
  <DesktopChrome platform="windows">
    <!-- LEFT: app name -->
    <template #titlebar-leading>
      <div class="appname">Horologion</div>
    </template>

    <!-- RIGHT: ARK status + settings (before window controls) -->
    <template #titlebar-trailing>
      <div class="trail">
        <button
          type="button"
          class="iconbtn iconbtn--titlebar"
          :title="arkStatusMessage"
          :aria-label="arkStatusMessage"
        >
          <span :class="arkDotClass" />
        </button>
        <RouterLink
          to="/settings"
          class="iconbtn iconbtn--titlebar"
          :class="{ 'iconbtn--active': isTab('settings') }"
          title="Настройки"
        >
          <Settings :size="16" :stroke-width="1.7" />
        </RouterLink>
      </div>
    </template>

    <DesktopContentSurface :padding-top="'0'" :padding-inline="'0'" :padding-bottom="'0'" :scrollable="false">
      <!-- Input row: play button + input -->
      <div class="inputbar">
        <button
          class="playbtn"
          :class="{ 'playbtn--running': running }"
          :title="running ? `Остановить (${timerLabel})` : 'Старт'"
          @click="toggleTimer"
        >
          <Pause v-if="running" :size="15" :stroke-width="2.3" />
          <Play v-else :size="15" :stroke-width="2.3" />
        </button>
        <div class="input-wrap">
          <input
            ref="inputRef"
            v-model="draft"
            class="inputbar__input"
            placeholder="@ для выбора задачи"
            @input="onInput"
            @keydown="onKeyDown"
          />
          <MentionMenu
            ref="mentionMenuRef"
            :open="mentionOpen"
            :query="mentionQuery"
            :tasks="tasks"
            :highlighted-index="mentionHighlight"
            @pick="pickTask"
            @hover="(i) => (mentionHighlight = i)"
          />
        </div>
        <span class="inputbar__timer" :class="{ 'inputbar__timer--running': running }">
          {{ timerLabel }}
        </span>
      </div>

      <nav class="tabs">
        <RouterLink
          v-for="t in tabs"
          :key="t.name"
          :to="t.path"
          class="tab"
          :class="{ 'tab--active': isTab(t.name) }"
        >
          <span class="tab__label">{{ t.label }}</span>
          <span class="tab__indicator" />
        </RouterLink>
      </nav>

      <main ref="contentRef" class="content" @scroll="onContentScroll">
        <div v-if="errMsg" class="errbar">⚠ {{ errMsg }}</div>
        <RouterView />
      </main>
    </DesktopContentSurface>
  </DesktopChrome>
</template>

<style scoped>
/* --- titlebar leading: app name --- */
.appname {
  display: inline-flex;
  align-items: center;
  height: 32px;
  font-size: 0.8125rem;
  font-weight: 600;
  letter-spacing: 0.01em;
  color: color-mix(in srgb, var(--sidebar-foreground) 55%, transparent);
  padding: 0 0.5rem;
  line-height: 1;
}

/* --- titlebar trailing: status + settings --- */
.trail {
  display: flex;
  align-items: center;
  gap: 0.25rem;
}

.iconbtn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  background: transparent;
  border: none;
  color: color-mix(in srgb, var(--sidebar-foreground) 55%, transparent);
  border-radius: 6px;
  cursor: pointer;
  text-decoration: none;
  transition:
    color 120ms cubic-bezier(0.2, 0, 0, 1),
    background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.iconbtn:hover {
  color: var(--sidebar-foreground);
  background: color-mix(in srgb, var(--sidebar-foreground) 8%, transparent);
}

.iconbtn--active {
  color: var(--accent);
}

.iconbtn--titlebar {
  width: 32px;
  height: 32px;
  border-radius: 10px;
  flex-shrink: 0;
}

/* ARK status dot — рендерится внутри обычной .iconbtn--titlebar кнопки. */
.dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: currentColor;
}

.dot--connected {
  color: var(--status-success);
}

.dot--connecting {
  /* Тёплый-жёлтый "warning"-токен ещё не объявлен в kosmos-visuals для всех тонов.
     Когда добавим - заменим на var(--status-warning). */
  color: oklch(0.75 0.14 75);
}

.dot--error {
  color: var(--destructive);
}

/* --- input row под titlebar'ом --- */
.inputbar {
  display: flex;
  align-items: center;
  gap: 0.625rem;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--border);
  background: var(--background);
}

.playbtn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: calc(var(--radius) * 0.75);
  corner-shape: var(--corner-shape);
  background: var(--accent);
  color: var(--accent-foreground);
  cursor: pointer;
  flex-shrink: 0;
  transition:
    background-color 220ms cubic-bezier(0.2, 0, 0, 1),
    color 220ms cubic-bezier(0.2, 0, 0, 1);
}

.playbtn:hover {
  background: var(--foreground);
  color: var(--background);
}

.playbtn--running {
  background: var(--destructive);
  color: var(--accent-foreground);
}

.playbtn--running:hover {
  background: var(--foreground);
  color: var(--background);
}

.input-wrap {
  position: relative;
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  height: 32px;
  padding: 0 0.75rem;
  background: color-mix(in srgb, var(--foreground) 5%, var(--background));
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.75);
  corner-shape: var(--corner-shape);
  transition:
    border-color 120ms cubic-bezier(0.2, 0, 0, 1),
    background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.input-wrap:focus-within {
  border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  background: var(--background);
}

.inputbar__input {
  flex: 1;
  min-width: 0;
  background: transparent;
  border: none;
  outline: none;
  color: var(--foreground);
  font-size: 0.875rem;
  font-weight: 500;
  letter-spacing: 0.01em;
}

.inputbar__input::placeholder {
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
}

.inputbar__timer {
  font-family: var(--font-mono);
  font-size: 0.875rem;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.02em;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  min-width: 64px;
  text-align: right;
  flex-shrink: 0;
}

.inputbar__timer--running {
  color: var(--foreground);
}

/* --- tabs (full width, 50/50) --- */
.tabs {
  display: flex;
  align-items: stretch;
  gap: 0;
  padding: 0;
  border-bottom: 1px solid var(--border);
  background: var(--background);
}

.tab {
  position: relative;
  flex: 1 1 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 36px;
  padding: 0;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  text-decoration: none;
  font-size: 0.8125rem;
  font-weight: 500;
  transition:
    color 160ms cubic-bezier(0.2, 0, 0, 1),
    background-color 160ms cubic-bezier(0.2, 0, 0, 1);
}

.tab__label {
  position: relative;
  z-index: 1;
}

.tab__indicator {
  position: absolute;
  bottom: -1px;
  left: 0;
  right: 0;
  height: 2px;
  background: transparent;
  transition: background-color 160ms cubic-bezier(0.2, 0, 0, 1);
}

.tab:hover {
  color: var(--foreground);
  background: color-mix(in srgb, var(--foreground) 3%, transparent);
}

.tab--active {
  color: var(--foreground);
}

.tab--active .tab__indicator {
  background: var(--accent);
}

/* --- content (скролл живёт здесь, не на всей странице — titlebar/tabs не двигаются) --- */
.content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  /* Слева 1rem, справа 0.5rem — компенсируем 8px scrollbar-gutter, чтобы
     визуально получилось ровно по линии с .inputbar (1rem с обеих сторон). */
  padding: 0.875rem 0.5rem 1.5rem 1rem;
  scrollbar-gutter: stable;
  /* По дефолту прозрачный — JS поднимает в 1 при скролле, плавно опускает
     в 0 после 500ms idle через rAF. */
  --sb-alpha: 0;
}

.content::-webkit-scrollbar {
  width: 8px;
}
.content::-webkit-scrollbar-track {
  background: transparent;
}
/* Фон thumb'а вычисляется из --sb-alpha. При 0 — полностью прозрачный
   (color-mix(..., 0%, transparent) = transparent), при 1 — обычный 30%. */
.content::-webkit-scrollbar-thumb {
  background-color: color-mix(
    in srgb,
    var(--foreground) calc(30% * var(--sb-alpha)),
    transparent
  );
  border-radius: 999px;
  border: 2px solid transparent;
  background-clip: padding-box;
}
.content::-webkit-scrollbar-thumb:hover {
  background-color: color-mix(
    in srgb,
    var(--foreground) calc(50% * var(--sb-alpha)),
    transparent
  );
}

.errbar {
  background: color-mix(in srgb, var(--destructive) 22%, var(--background));
  color: color-mix(in srgb, var(--destructive) 95%, var(--foreground));
  border: 1px solid color-mix(in srgb, var(--destructive) 40%, transparent);
  padding: 0.5rem 0.75rem;
  border-radius: 8px;
  margin-bottom: 0.75rem;
  font-size: 0.8125rem;
}
</style>
