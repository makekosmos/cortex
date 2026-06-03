<script setup lang="ts">
// Textarea — multi-line input primitive, визуально согласован с TextInput
// и Dropdown. Авто-resize отключён (controlled через `rows` + CSS resize).

import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    placeholder?: string;
    disabled?: boolean;
    readonly?: boolean;
    rows?: number;
    /** Минимальная высота в px (override CSS default). */
    minHeight?: number;
    /** CSS resize policy. */
    resize?: "none" | "vertical" | "horizontal" | "both";
    invalid?: boolean;
  }>(),
  {
    disabled: false,
    readonly: false,
    rows: 3,
    resize: "vertical",
    invalid: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [v: string];
  blur: [e: FocusEvent];
  focus: [e: FocusEvent];
}>();

const value = computed({
  get: () => props.modelValue,
  set: (v) => emit("update:modelValue", v),
});

const normalizedMinHeight = computed(() =>
  props.minHeight ? Math.max(8, Math.round(props.minHeight / 8) * 8) : undefined,
);
</script>

<template>
  <textarea
    v-model="value"
    :placeholder="placeholder"
    :disabled="disabled"
    :readonly="readonly"
    :rows="rows"
    :class="[
      'w-full rounded-lg border-2 border-solid border-[var(--border)] bg-[color-mix(in_srgb,var(--foreground)_4%,var(--background))] px-4 py-2 font-[inherit] text-sm leading-normal text-[var(--foreground)] transition-[border-color,background-color] duration-140 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] placeholder:text-[color-mix(in_srgb,var(--foreground)_45%,transparent)] hover:not-disabled:not-focus:border-[color-mix(in_srgb,var(--accent)_30%,var(--border))] focus:border-[color-mix(in_srgb,var(--accent)_65%,transparent)] focus:outline-none disabled:cursor-not-allowed disabled:opacity-55',
      invalid ? 'border-[var(--destructive)] focus:border-[var(--destructive)]' : '',
    ]"
    :style="{
      resize,
      minHeight: normalizedMinHeight ? `${normalizedMinHeight}px` : undefined,
    }"
    @blur="(e) => emit('blur', e)"
    @focus="(e) => emit('focus', e)"
  />
</template>
