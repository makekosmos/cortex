<script setup lang="ts">
// HotkeyCapture — UI-only компонент для захвата accelerator-сочетания.
// При клике переходит в режим "ждёт нажатие", собирает modifiers + key,
// эмитит `update:modelValue` со строкой вида "Ctrl+Shift+;" и автоматически
// выходит из режима. Escape — отменяет.
//
// Регистрация / валидация в системе — задача parent'а (через emit).

import { ref } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    /** Текст в плейсхолдере когда нет hotkey. */
    placeholder?: string;
    /** Текст пока ждём нажатие. */
    capturePrompt?: string;
    disabled?: boolean;
  }>(),
  {
    placeholder: "Не задано",
    capturePrompt: "Нажмите сочетание…",
    disabled: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [v: string];
  /** Срабатывает при Escape — parent может отреагировать (например clear). */
  cancel: [];
}>();

const capturing = ref(false);

function start() {
  if (props.disabled) return;
  capturing.value = true;
}

function stop() {
  capturing.value = false;
}

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
