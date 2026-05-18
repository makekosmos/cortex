<script setup lang="ts">
// Floating focus widget — всегда поверх остальных окон, маленькое окно
// ~320x52. Слева MM:SS countdown, справа подпись над чем работаем.
// Лayout вдохновлён Spotify mini-player + Raycast Focus mode.
//
// Получает state через IPC `kepler:focus-widget:state` события от main
// (Horologion публикует обновления через `window.kepler.focusWidget.setState`).

import { computed, onBeforeUnmount, onMounted, ref } from "vue";

interface FocusState {
  active: boolean;
  remainingSec: number;
  label: string;
  /** 'work' (red accent) | 'break' (green accent) | 'stopwatch' (neutral). */
  mode: "work" | "break" | "stopwatch";
  /** Применён ли активный блоклист — 🛡️ индикатор показывается слева от времени. */
  blockingActive: boolean;
}

const state = ref<FocusState>({
  active: false,
  remainingSec: 0,
  label: "",
  mode: "work",
  blockingActive: false,
});

let unsubscribe: (() => void) | null = null;

onMounted(() => {
  // Subscribe to state updates from main process.
  unsubscribe = window.kepler.focusWidget.onState((s) => {
    state.value = s;
  });
  // Initial fetch (in case widget opened mid-session).
  window.kepler.focusWidget.getState().then((s) => {
    if (s) state.value = s;
  }).catch(() => { /* ignore */ });
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

function onClick(): void {
  window.kepler.focusWidget.openHorologion?.();
}

function onClose(): void {
  window.kepler.focusWidget.hide?.();
}
</script>

<template>
  <div class="widget" :class="modeClass">
    <!-- Drag handle covers most of the widget -->
    <div class="drag-area" @dblclick="onClick">
      <span
        v-if="state.blockingActive"
        class="shield"
        :title="'Блокировка активна'"
        aria-label="Блокировка активна"
      >🛡️</span>
      <div class="time">{{ timeText }}</div>
      <div class="separator" />
      <div class="label" :title="labelText">{{ labelText }}</div>
    </div>
    <button
      class="close-btn"
      type="button"
      :title="'Скрыть виджет'"
      @click="onClose"
    >
      ×
    </button>
  </div>
</template>

<style scoped>
.widget {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  padding: 0 6px 0 16px;
  background: color-mix(in srgb, var(--background) 70%, transparent);
  color: var(--foreground);
  font-family: var(--font-sans);
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  box-shadow: 0 4px 24px color-mix(in srgb, #000 28%, transparent);
  user-select: none;
  overflow: hidden;
}

/* Accent stripe slim слева — visual mode indicator */
.widget::before {
  content: "";
  position: absolute;
  inset: 0 auto 0 0;
  width: 3px;
  background: var(--accent-stripe, var(--primary));
}
.mode-work {
  --accent-stripe: #e74c3c;
}
.mode-break {
  --accent-stripe: #2ecc71;
}
.mode-stopwatch {
  --accent-stripe: var(--primary);
}

.drag-area {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 12px;
  height: 100%;
  -webkit-app-region: drag;
  cursor: grab;
}

.shield {
  font-size: 14px;
  line-height: 1;
  opacity: 0.85;
  margin-right: -4px;
}

.time {
  font-variant-numeric: tabular-nums;
  font-size: 22px;
  font-weight: 600;
  letter-spacing: -0.02em;
  color: var(--foreground);
  min-width: 70px;
}

.separator {
  width: 1px;
  height: 22px;
  background: color-mix(in srgb, var(--border) 90%, transparent);
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

.close-btn {
  -webkit-app-region: no-drag;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  background: transparent;
  border: none;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 18px;
  line-height: 1;
  border-radius: 6px;
  cursor: pointer;
  transition: background 100ms, color 100ms;
}
.close-btn:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}
</style>
