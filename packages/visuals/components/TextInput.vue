<script setup lang="ts">
// TextInput — single-line input primitive, визуально согласован с Dropdown
// (та же высота, бордеры, hover/focus). Поддерживает type=text/password/email/url
// и события v-model.

import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    type?: "text" | "password" | "email" | "url" | "search";
    placeholder?: string;
    disabled?: boolean;
    readonly?: boolean;
    /** Размер: md (default) / sm. */
    size?: "md" | "sm";
    /** Полная ширина контейнера. */
    block?: boolean;
    /** Visual error / invalid state. */
    invalid?: boolean;
    /** Автокомплит (для password — "current-password" / "new-password"). */
    autocomplete?: string;
    /** Inputmode для виртуальных клавиатур. */
    inputmode?: "text" | "numeric" | "decimal" | "tel" | "search" | "email" | "url";
  }>(),
  {
    type: "text",
    disabled: false,
    readonly: false,
    size: "md",
    block: true,
    invalid: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [v: string];
  blur: [e: FocusEvent];
  focus: [e: FocusEvent];
  keydown: [e: KeyboardEvent];
}>();

const value = computed({
  get: () => props.modelValue,
  set: (v) => emit("update:modelValue", v),
});
</script>

<template>
  <input
    v-model="value"
    :type="type"
    :placeholder="placeholder"
    :disabled="disabled"
    :readonly="readonly"
    :autocomplete="autocomplete"
    :inputmode="inputmode"
    :class="[
      'kosmos-input',
      `kosmos-input--${size}`,
      { 'kosmos-input--block': block, 'kosmos-input--invalid': invalid },
    ]"
    @blur="(e) => emit('blur', e)"
    @focus="(e) => emit('focus', e)"
    @keydown="(e) => emit('keydown', e)"
  />
</template>

<style scoped>
.kosmos-input {
  height: 34px;
  padding: 0 0.625rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 2px solid var(--border);
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.875rem;
  line-height: 1.4;
  transition:
    border-color 140ms cubic-bezier(0.2, 0, 0, 1),
    background-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-input--sm {
  height: 28px;
  font-size: 0.8125rem;
}

.kosmos-input--block {
  width: 100%;
}

.kosmos-input:hover:not(:disabled):not(:focus) {
  border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
}

.kosmos-input:focus {
  outline: none;
  border-color: color-mix(in srgb, var(--accent) 65%, transparent);
}

.kosmos-input:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.kosmos-input--invalid {
  border-color: var(--destructive);
}

.kosmos-input::placeholder {
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
}
</style>
