<script setup lang="ts">
// Toggle / switch — бинарный control. Используется в settings (Horologion,
// Delphi, Eden) для опций on/off.
//
// API совместим с v-model: `:model-value` / `@update:modelValue`.

import { computed } from "vue";

interface Props {
  modelValue: boolean;
  /** Текстовая подпись справа от switch'а (опционально — обычно label
   * приходит из обёртки SettingsRow). */
  label?: string;
  disabled?: boolean;
  /** Аккессибильное имя для скрин-ридеров если нет visible label'а. */
  ariaLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
});

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
}>();

const checked = computed(() => props.modelValue);

function toggle() {
  if (props.disabled) return;
  emit("update:modelValue", !checked.value);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === " " || e.key === "Enter") {
    e.preventDefault();
    toggle();
  }
}
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="checked"
    :aria-label="ariaLabel ?? label"
    :disabled="disabled"
    class="kosmos-toggle"
    :class="{
      'kosmos-toggle--checked': checked,
      'kosmos-toggle--disabled': disabled,
    }"
    @click="toggle"
    @keydown="onKeydown"
  >
    <span class="kosmos-toggle__track">
      <span class="kosmos-toggle__thumb" />
    </span>
    <span v-if="label" class="kosmos-toggle__label">{{ label }}</span>
  </button>
</template>

<style scoped>
.kosmos-toggle {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0;
  border: none;
  background: transparent;
  cursor: default;
  font: inherit;
  color: inherit;
  outline-offset: 2px;
}

.kosmos-toggle:focus-visible {
  outline: 2px solid var(--ring);
  border-radius: 999px;
}

.kosmos-toggle__track {
  position: relative;
  width: 36px;
  height: 20px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--muted-foreground) 35%, transparent);
  transition: background-color 140ms cubic-bezier(0.2, 0, 0, 1);
  flex-shrink: 0;
}

.kosmos-toggle__thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 999px;
  background: var(--background);
  transition: transform 140ms cubic-bezier(0.2, 0, 0, 1);
  box-shadow: 0 1px 3px color-mix(in srgb, #000 25%, transparent);
}

.kosmos-toggle--checked .kosmos-toggle__track {
  background: var(--primary);
}

.kosmos-toggle--checked .kosmos-toggle__thumb {
  transform: translateX(16px);
}

.kosmos-toggle--disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.kosmos-toggle__label {
  font-size: 0.875rem;
  color: var(--foreground);
}
</style>
