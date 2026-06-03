<script setup lang="ts">
// Square checkbox — единый primitive для всех «галочек» в экосистеме
// (Eden TaskRef, Delphi todo subitems, settings и т.д.).
//
// Style: outline (24×24, 8px radius, 2px border) + inset filled square
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
    :class="[
      'relative inline-flex size-6 shrink-0 cursor-default items-center justify-center rounded-lg border-2 border-solid border-[var(--ring)] bg-transparent p-0 outline-offset-2 transition-[border-color,background-color] duration-150 hover:not-disabled:border-[var(--kosmos-checkbox-accent,var(--accent))] focus-visible:outline-2 focus-visible:outline-[var(--kosmos-checkbox-accent,var(--accent))] disabled:cursor-not-allowed disabled:opacity-50',
      checked ? 'border-[var(--kosmos-checkbox-accent,var(--accent))]' : '',
    ]"
    @click="toggle"
    @keydown="onKeydown"
  >
    <span
      v-if="checked"
      class="absolute inset-2 block rounded-none bg-[var(--kosmos-checkbox-accent,var(--accent))]"
      aria-hidden="true"
    />
  </button>
</template>
