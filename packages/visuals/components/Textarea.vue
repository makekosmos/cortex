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
</script>

<template>
  <textarea
    v-model="value"
    :placeholder="placeholder"
    :disabled="disabled"
    :readonly="readonly"
    :rows="rows"
    :class="['kosmos-textarea', { 'kosmos-textarea--invalid': invalid }]"
    :style="{
      resize,
      minHeight: minHeight ? `${minHeight}px` : undefined,
    }"
    @blur="(e) => emit('blur', e)"
    @focus="(e) => emit('focus', e)"
  />
</template>

<style scoped>
.kosmos-textarea {
  width: 100%;
  padding: 0.5rem 0.625rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 2px solid var(--border);
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.875rem;
  line-height: 1.5;
  transition:
    border-color 140ms cubic-bezier(0.2, 0, 0, 1),
    background-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-textarea:hover:not(:disabled):not(:focus) {
  border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
}

.kosmos-textarea:focus {
  outline: none;
  border-color: color-mix(in srgb, var(--accent) 65%, transparent);
}

.kosmos-textarea:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.kosmos-textarea--invalid {
  border-color: var(--destructive);
}

.kosmos-textarea::placeholder {
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
}
</style>
