<script setup lang="ts">
// Square checkbox — единый primitive для всех «галочек» в экосистеме
// (Eden TaskRef, Delphi todo subitems, settings и т.д.).
//
// Style: outline (18×18, 6px radius, 2px border) + inset filled square
// при checked. Без ✓ glyph'а. Совпадает с Delphi TodoRow `.check-box` —
// единое визуальное представление task'а в экосистеме.
//
// API совместим с v-model: `:model-value` / `@update:modelValue`.
//
// Цвет акцента переопределяется через CSS var `--kosmos-checkbox-accent`
// на родителе — так Eden может подсунуть свой brand orange без хардкода
// `#hex` внутри компонента. Default — `var(--accent)`.

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
    <span v-if="checked" class="kosmos-checkbox__inner" aria-hidden="true" />
  </button>
</template>

<style scoped>
/* Delphi TodoRow `.check-box` parity: outline-style square с inner filled
   prefix. `--ring` для outline (нейтральный border при unchecked),
   `--kosmos-checkbox-accent` (override от родителя) или `--accent` для
   filled state и hover border. */
.kosmos-checkbox {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  border-radius: 6px;
  border: 2px solid var(--ring);
  background: transparent;
  cursor: pointer;
  transition:
    border-color 0.15s,
    background-color 0.15s;
  outline-offset: 2px;
  flex-shrink: 0;
  position: relative;
}

.kosmos-checkbox:focus-visible {
  outline: 2px solid var(--kosmos-checkbox-accent, var(--accent));
}

.kosmos-checkbox:hover:not(.kosmos-checkbox--disabled) {
  border-color: var(--kosmos-checkbox-accent, var(--accent));
}

.kosmos-checkbox--checked {
  border-color: var(--kosmos-checkbox-accent, var(--accent));
}

.kosmos-checkbox--disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

/* Inner filled square — Delphi `.check-box__inner` parity: inset 2px от
   outline rim, корпус radius 3px. */
.kosmos-checkbox__inner {
  display: block;
  position: absolute;
  inset: 2px;
  border-radius: 3px;
  background-color: var(--kosmos-checkbox-accent, var(--accent));
}
</style>
