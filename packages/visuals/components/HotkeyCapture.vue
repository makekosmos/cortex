<script setup lang="ts">
// HotkeyCapture — UI primitive для назначения accelerator'а.
//
// Два режима capture:
//   1. Локальный (default) — слушает DOM keydown сам. Работает для
//      несистемных shortcut'ов (Ctrl+Shift+;, Alt+D, etc.).
//   2. Внешний — компонент только показывает UI «жду нажатие», а событие
//      приходит через prop'ы. Используется когда нужно ловить системные
//      shortcut'ы (Win+H, Win+Space) через нижестоящий keyboard hook
//      ОС-уровня — иначе системный shortcut срабатывает раньше WebContents
//      keyboard handler'а.
//
// Внешний режим включается передачей prop `externalCapture: true`. В этом
// случае parent сам стартует capture в системе (например через
// `dictation.begin_hotkey_capture` backend op'у) и передаёт результат
// обратно через `pendingAccelerator` / `pendingCancel` events.

import { ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    /** Текст в плейсхолдере когда нет hotkey. */
    placeholder?: string;
    /** Текст пока ждём нажатие. */
    capturePrompt?: string;
    disabled?: boolean;
    /** Если true — компонент НЕ слушает DOM keydown сам. Capture лифт
     * наружу: parent стартует системный hook (например через ARK
     * `dictation.begin_hotkey_capture`) когда срабатывает `@capture-start`,
     * прокидывает результат назад установкой `pendingAccelerator`. */
    externalCapture?: boolean;
    /** Внешне-полученный accelerator (от parent'а в externalCapture mode).
     * При изменении на не-пустую строку — emit'им `update:modelValue` и
     * выходим из capture state. */
    pendingAccelerator?: string | null;
    /** Cancel signal от parent'а (например юзер нажал Escape — backend
     * прислал `dictation_capture_cancelled`). Toggle для тригера. */
    pendingCancel?: number;
  }>(),
  {
    placeholder: "Не задано",
    capturePrompt: "Нажмите сочетание…",
    disabled: false,
    externalCapture: false,
    pendingAccelerator: null,
    pendingCancel: 0,
  },
);

const emit = defineEmits<{
  "update:modelValue": [v: string];
  /** Срабатывает при Escape — parent может отреагировать (например clear). */
  cancel: [];
  /** В externalCapture mode — parent должен стартовать системный hook. */
  "capture-start": [];
  /** В externalCapture mode — parent должен остановить системный hook
   *  (например при blur'е окна или unmount). */
  "capture-end": [];
}>();

const capturing = ref(false);

function start() {
  if (props.disabled) return;
  capturing.value = true;
  if (props.externalCapture) {
    emit("capture-start");
  }
}

function stop() {
  if (capturing.value && props.externalCapture) {
    emit("capture-end");
  }
  capturing.value = false;
}

// External capture: реактивно ловим accelerator или cancel от parent'а.
watch(
  () => props.pendingAccelerator,
  (v) => {
    if (!props.externalCapture) return;
    if (v && capturing.value) {
      emit("update:modelValue", v);
      capturing.value = false;
    }
  },
);
watch(
  () => props.pendingCancel,
  () => {
    if (!props.externalCapture) return;
    if (capturing.value) {
      capturing.value = false;
      emit("cancel");
    }
  },
);

function keyEventToAccelerator(e: KeyboardEvent): string | null {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  const key = e.key;
  if (key === "Control" || key === "Alt" || key === "Shift" || key === "Meta") {
    return null;
  }
  let main: string;
  if (key === " ") main = "Space";
  else if (key === "Escape") return "ESC_CANCEL";
  else if (key.length === 1) main = key.toUpperCase();
  else main = key;
  parts.push(main);
  return parts.join("+");
}

function onKey(e: KeyboardEvent) {
  if (!capturing.value) return;
  // В externalCapture parent сам ловит события через системный hook —
  // DOM keydown не trustworthy (системные shortcut'ы перехватываются до
  // того как WebContents получит event).
  if (props.externalCapture) return;
  e.preventDefault();
  e.stopPropagation();
  const acc = keyEventToAccelerator(e);
  if (!acc) return;
  if (acc === "ESC_CANCEL") {
    capturing.value = false;
    emit("cancel");
    return;
  }
  emit("update:modelValue", acc);
  capturing.value = false;
}
</script>

<template>
  <button
    type="button"
    :class="['kosmos-hk', { 'kosmos-hk--capturing': capturing, 'kosmos-hk--disabled': disabled }]"
    :disabled="disabled"
    @click="start"
    @keydown="onKey"
    @blur="stop"
  >
    <span v-if="capturing" class="kosmos-hk__prompt">{{ capturePrompt }}</span>
    <code v-else-if="modelValue" class="kosmos-hk__value">{{ modelValue }}</code>
    <span v-else class="kosmos-hk__placeholder">{{ placeholder }}</span>
  </button>
</template>

<style scoped>
.kosmos-hk {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 140px;
  height: 34px;
  padding: 0 0.75rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 2px solid var(--border);
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.875rem;
  cursor: pointer;
  transition:
    border-color 140ms cubic-bezier(0.2, 0, 0, 1),
    background-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-hk:hover:not(.kosmos-hk--disabled):not(.kosmos-hk--capturing) {
  border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
}

.kosmos-hk--capturing {
  border-color: color-mix(in srgb, var(--accent) 65%, transparent);
  background: color-mix(in srgb, var(--accent) 7%, var(--background));
}

.kosmos-hk--disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.kosmos-hk__value {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.8125rem;
  letter-spacing: 0.02em;
  color: var(--foreground);
}

.kosmos-hk__prompt {
  color: var(--accent);
  font-weight: 500;
}

.kosmos-hk__placeholder {
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
}
</style>
