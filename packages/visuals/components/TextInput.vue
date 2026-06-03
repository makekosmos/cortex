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
      'rounded-lg border-2 border-solid border-[var(--border)] bg-[color-mix(in_srgb,var(--foreground)_4%,var(--background))] px-4 font-[inherit] text-sm leading-[1.4] text-[var(--foreground)] transition-[border-color,background-color] duration-140 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] placeholder:text-[color-mix(in_srgb,var(--foreground)_45%,transparent)] hover:not-disabled:not-focus:border-[color-mix(in_srgb,var(--accent)_30%,var(--border))] focus:border-[color-mix(in_srgb,var(--accent)_65%,transparent)] focus:outline-none disabled:cursor-not-allowed disabled:opacity-55',
      size === 'sm' ? 'h-8 px-2 text-[0.8125rem]' : 'h-10',
      block ? 'w-full' : '',
      invalid ? 'border-[var(--destructive)] focus:border-[var(--destructive)]' : '',
    ]"
    @blur="(e) => emit('blur', e)"
    @focus="(e) => emit('focus', e)"
    @keydown="(e) => emit('keydown', e)"
  />
</template>
