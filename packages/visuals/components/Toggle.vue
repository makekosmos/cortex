<script setup lang="ts">
// Toggle / switch — бинарный control. Используется в settings
// Delphi/Eden для опций on/off.
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
    class="inline-flex cursor-default items-center gap-2 border-0 bg-transparent p-0 font-[inherit] text-[inherit] outline-offset-2 focus-visible:rounded-full focus-visible:outline-2 focus-visible:outline-[var(--ring)] disabled:cursor-not-allowed disabled:opacity-50"
    @click="toggle"
    @keydown="onKeydown"
  >
    <span
      :class="[
        'relative h-5 w-10 shrink-0 rounded-full bg-[color-mix(in_srgb,var(--foreground)_18%,transparent)] transition-[background-color] duration-150 ease-[cubic-bezier(0.2,0,0,1)]',
        checked ? 'bg-[var(--accent)]' : '',
      ]"
    >
      <span
        :class="[
          'absolute top-0.5 left-0.5 size-4 rounded-full bg-[var(--foreground)] shadow-[0_1px_3px_color-mix(in_srgb,var(--background)_24%,transparent)] transition-transform duration-150 ease-[cubic-bezier(0.2,0,0,1)]',
          checked ? 'translate-x-5' : '',
        ]"
      />
    </span>
    <span v-if="label" class="text-sm text-[var(--foreground)]">{{ label }}</span>
  </button>
</template>
