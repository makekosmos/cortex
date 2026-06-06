<script setup lang="ts">
// Floating focus widget — всегда поверх остальных окон, маленькое окно
// ~320x52. Слева MM:SS countdown, справа подпись над чем работаем.
// Лayout вдохновлён Spotify mini-player + Raycast Focus mode.
//
// Получает state через IPC `kepler:focus-widget:state` события от main
// (Horologion публикует обновления через `window.kepler.focusWidget.setState`).

import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { GripVertical, Pause, Play, Check, MoreVertical } from "@lucide/vue";
import { IconButton } from "@kosmos/visuals";

interface FocusState {
  active: boolean;
  remainingSec: number;
  totalSec: number;
  label: string;
  /** 'work' (red accent) | 'break' (green accent) | 'stopwatch' (neutral). */
  mode: "work" | "break" | "stopwatch";
  /** Применён ли активный blocklist/app block. UI не показывает отдельный значок. */
  blockingActive: boolean;
  /** Pomodoro session на паузе — кнопка показывает Play вместо Pause. */
  isPaused: boolean;
}

const state = ref<FocusState>({
  active: false,
  remainingSec: 0,
  totalSec: 0,
  label: "",
  mode: "work",
  blockingActive: false,
  isPaused: false,
});

let unsubscribe: (() => void) | null = null;

onMounted(() => {
  unsubscribe = window.kepler.focusWidget.onState((s) => {
    state.value = s;
  });
  window.kepler.focusWidget
    .getState()
    .then((s) => {
      if (s) state.value = s;
    })
    .catch(() => {
      /* ignore */
    });
});

onBeforeUnmount(() => {
  unsubscribe?.();
});

const timeText = computed(() => {
  const total = Math.max(0, Math.floor(state.value.remainingSec));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
});

const labelText = computed(() => {
  const l = state.value.label.trim();
  if (l) return l;
  if (state.value.mode === "break") return "Перерыв";
  if (state.value.mode === "stopwatch") return "Секундомер";
  return "Фокус";
});

const modeClass = computed(() => `mode-${state.value.mode}`);

const progressStyle = computed(() => {
  const total = Math.max(0, state.value.totalSec);
  if (total <= 0 || state.value.mode === "stopwatch") {
    return { "--progress": "0%" };
  }
  const remaining = Math.min(total, Math.max(0, state.value.remainingSec));
  return { "--progress": `${Math.round((1 - remaining / total) * 100)}%` };
});

function onClick(): void {
  window.kepler.focusWidget.openHorologion?.();
}

const showControls = computed(() => state.value.active);
const isPomodoro = computed(() => state.value.mode !== "stopwatch");

async function onPauseToggle(): Promise<void> {
  const api = window.kepler.focusWidget;
  try {
    if (state.value.isPaused) {
      await api.pomodoro.resume();
    } else {
      await api.pomodoro.pause();
    }
  } catch (e) {
    console.error("[focus-widget] pause toggle failed:", e);
  }
}

async function onStop(): Promise<void> {
  const api = window.kepler.focusWidget;
  try {
    if (isPomodoro.value) {
      await api.pomodoro.stop();
    } else {
      await api.stopwatch.stop();
    }
  } catch (e) {
    console.error("[focus-widget] stop failed:", e);
  }
}

async function onShowMenu(): Promise<void> {
  try {
    await window.kepler.focusWidget.showMenu();
  } catch (e) {
    console.error("[focus-widget] show menu failed:", e);
  }
}
</script>

<template>
  <div class="widget" :class="modeClass" :style="progressStyle">
    <div class="progress-fill" aria-hidden="true" />
    <div class="content" @dblclick="onClick">
      <div class="time">{{ timeText }}</div>
      <div class="label" :title="labelText">{{ labelText }}</div>
    </div>
    <div class="actions" aria-label="Действия виджета">
      <div v-if="showControls" class="controls">
        <button
          type="button"
          class="btn"
          :aria-label="state.isPaused ? 'Продолжить' : 'Пауза'"
          @click="onPauseToggle"
        >
          <component :is="state.isPaused ? Play : Pause" :size="14" />
          <span>{{ state.isPaused ? "Продолжить" : "Пауза" }}</span>
        </button>
        <button type="button" class="btn" aria-label="Выполнено" @click="onStop">
          <Check :size="14" />
          <span>Выполнено</span>
        </button>
        <IconButton :size="24" title="Ещё" aria-label="Ещё" @click="onShowMenu">
          <MoreVertical :size="14" />
        </IconButton>
      </div>
    </div>
    <div class="drag-handle" title="Переместить виджет" aria-label="Переместить виджет">
      <GripVertical :size="15" />
    </div>
  </div>
</template>

<style scoped>
.widget {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  padding: 0 9px;
  background: color-mix(in srgb, var(--popover) 96%, transparent);
  color: var(--foreground);
  font-family: var(--font-sans);
  border-radius: 8px;
  border: 1px solid color-mix(in srgb, var(--border) 82%, transparent);
  box-shadow:
    0 14px 32px color-mix(in srgb, var(--background) 42%, transparent),
    0 0 0 1px color-mix(in srgb, var(--foreground) 5%, transparent) inset;
  user-select: none;
  overflow: hidden;
  -webkit-app-region: no-drag;
  backdrop-filter: blur(18px);
  transition:
    background-color 140ms cubic-bezier(0.2, 0, 0, 1),
    border-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.mode-work {
  --progress-color: var(--timer-work);
}
.mode-break {
  --progress-color: var(--timer-break);
}
.mode-stopwatch {
  --progress-color: var(--timer-stopwatch);
}

.progress-fill {
  position: absolute;
  inset: 0 auto 0 0;
  width: var(--progress, 0%);
  min-width: 0;
  background: color-mix(in srgb, var(--progress-color, var(--primary)) 22%, transparent);
  transition: width 420ms linear;
  pointer-events: none;
}

.drag-handle {
  position: absolute;
  right: 4px;
  top: 0;
  bottom: 0;
  z-index: 4;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  color: color-mix(in srgb, var(--foreground) 38%, transparent);
  -webkit-app-region: drag;
  cursor: grab;
  opacity: 0;
  pointer-events: none;
  transition: opacity 120ms cubic-bezier(0.2, 0, 0, 1);
}

.widget:hover .drag-handle,
.widget:focus-within .drag-handle {
  opacity: 1;
  pointer-events: auto;
}

.content {
  position: relative;
  z-index: 1;
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  height: 100%;
  -webkit-app-region: no-drag;
  cursor: default;
  transition:
    opacity 120ms cubic-bezier(0.2, 0, 0, 1),
    transform 120ms cubic-bezier(0.2, 0, 0, 1);
}

.time {
  font-variant-numeric: tabular-nums;
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 600;
  color: var(--foreground);
  background: color-mix(in srgb, var(--muted) 45%, transparent);
  padding: 2px 6px;
  border-radius: 4px;
}

.label {
  flex: 1;
  font-size: 13px;
  font-weight: 500;
  color: color-mix(in srgb, var(--foreground) 85%, transparent);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.controls {
  display: flex;
  align-items: center;
  gap: 4px;
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 24px;
  padding: 0 6px;
  background: transparent;
  border: none;
  border-radius: 4px;
  color: var(--foreground);
  font-family: inherit;
  font-size: 12px;
  font-weight: 500;
  cursor: default;
  white-space: nowrap;
  transition: background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.btn:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.btn:active {
  background: color-mix(in srgb, var(--foreground) 14%, transparent);
}

.actions {
  position: absolute;
  z-index: 3;
  inset: 0 32px 0 9px;
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 2px;
  opacity: 0;
  pointer-events: none;
  transform: translateY(2px);
  transition:
    opacity 120ms cubic-bezier(0.2, 0, 0, 1),
    transform 120ms cubic-bezier(0.2, 0, 0, 1);
  -webkit-app-region: no-drag;
}

.widget:hover,
.widget:focus-within {
  background: color-mix(in srgb, var(--popover) 99%, transparent);
  border-color: color-mix(in srgb, var(--border) 92%, transparent);
}

.widget:hover .content,
.widget:focus-within .content {
  opacity: 0;
  transform: translateY(-2px);
  pointer-events: none;
}

.widget:hover .actions,
.widget:focus-within .actions {
  opacity: 1;
  pointer-events: auto;
  transform: translateY(0);
}
</style>
