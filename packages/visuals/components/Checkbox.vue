<script setup lang="ts">
// Square checkbox — единый primitive для всех «галочек» в экосистеме
// (Eden TaskRef, Delphi todo subitems, settings и т.д.). Anytype-style:
// 18×18, 4px radius, ✓ glyph при checked.
//
// API совместим с v-model: `:model-value` / `@update:modelValue`.
//
// Цвет акцента переопределяется через CSS var `--kosmos-checkbox-accent`
// на родителе — так Eden может подсунуть свой brand orange без хардкода
// `#hex` внутри компонента. Default — `var(--primary)`.

import { computed } from "vue";

interface Props {
  modelValue: boolean;
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
    role="checkbox"
    :aria-checked="checked"
    :aria-label="ariaLabel"
    :disabled="disabled"
    class="kosmos-checkbox"
    :class="{
      'kosmos-checkbox--checked': checked,
      'kosmos-checkbox--disabled': disabled,
    }"
    @click="toggle"
    @keydown="onKeydown"
  >
    <span class="kosmos-checkbox__glyph" aria-hidden="true">
      <svg
        v-if="checked"
        viewBox="0 0 12 12"
        width="12"
        height="12"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M2.5 6.5 L5 9 L9.5 3.5" />
      </svg>
    </span>
  </button>
</template>

<style scoped>
.kosmos-checkbox {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  border-radius: 4px;
  border: 1.5px solid var(--border);
  background: transparent;
  cursor: pointer;
  color: var(--accent-foreground, var(--background));
  transition:
    background-color 120ms ease,
    border-color 120ms ease,
    color 120ms ease;
  outline-offset: 2px;
  flex-shrink: 0;
}

.kosmos-checkbox:focus-visible {
  outline: 2px solid var(--kosmos-checkbox-accent, var(--primary));
  border-radius: 4px;
}

.kosmos-checkbox:hover:not(.kosmos-checkbox--disabled) {
  border-color: var(--kosmos-checkbox-accent, var(--primary));
}

.kosmos-checkbox--checked {
  background: var(--kosmos-checkbox-accent, var(--primary));
  border-color: var(--kosmos-checkbox-accent, var(--primary));
}

.kosmos-checkbox--disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.kosmos-checkbox__glyph {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 0;
}
</style>
