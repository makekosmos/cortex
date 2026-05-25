<script setup lang="ts" generic="T extends string | number">
// RadioGroup — вертикальный список радиокнопок с опциональным описанием
// под каждой опцией. Используется когда Dropdown слишком "сжимает" UX и
// хочется показать сразу все варианты с пояснениями.
//
// Стилистически — кастомные radio "пилюли" поверх токенов visuals,
// без `<input type=radio>` напрямую в UI (input спрятан для a11y).

import { computed } from "vue";

interface Option<V> {
  value: V;
  label: string;
  description?: string;
  disabled?: boolean;
}

const props = withDefaults(
  defineProps<{
    modelValue: T;
    options: Option<T>[];
    /** Имя группы — для уникальности radio name (default — random). */
    name?: string;
    /** Layout. Default "vertical". */
    direction?: "vertical" | "horizontal";
    disabled?: boolean;
  }>(),
  {
    direction: "vertical",
    disabled: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [v: T];
}>();

const groupName = computed(
  () => props.name ?? `kosmos-rg-${Math.random().toString(36).slice(2, 8)}`,
);

function pick(opt: Option<T>) {
  if (props.disabled || opt.disabled) return;
  if (opt.value === props.modelValue) return;
  emit("update:modelValue", opt.value);
}
</script>

<template>
  <div
    :class="['kosmos-rg', `kosmos-rg--${direction}`, { 'kosmos-rg--disabled': disabled }]"
    role="radiogroup"
  >
    <label
      v-for="opt in options"
      :key="String(opt.value)"
      :class="[
        'kosmos-rg__item',
        {
          'kosmos-rg__item--selected': opt.value === modelValue,
          'kosmos-rg__item--disabled': opt.disabled || disabled,
        },
      ]"
    >
      <input
        type="radio"
        :name="groupName"
        :value="opt.value"
        :checked="opt.value === modelValue"
        :disabled="opt.disabled || disabled"
        class="kosmos-rg__native"
        @change="pick(opt)"
      />
      <span class="kosmos-rg__dot" aria-hidden="true">
        <span class="kosmos-rg__dot-inner" />
      </span>
      <span class="kosmos-rg__text">
        <span class="kosmos-rg__label">{{ opt.label }}</span>
        <span v-if="opt.description" class="kosmos-rg__desc">{{ opt.description }}</span>
      </span>
    </label>
  </div>
</template>

<style scoped>
.kosmos-rg {
  display: flex;
  gap: 6px;
}

.kosmos-rg--vertical {
  flex-direction: column;
}

.kosmos-rg--horizontal {
  flex-direction: row;
  flex-wrap: wrap;
}

.kosmos-rg__item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 8px 10px;
  border: 2px solid var(--border);
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  background: color-mix(in srgb, var(--foreground) 3%, var(--background));
  cursor: default;
  transition:
    border-color 140ms cubic-bezier(0.2, 0, 0, 1),
    background-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-rg__item:hover:not(.kosmos-rg__item--disabled):not(.kosmos-rg__item--selected) {
  border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
}

.kosmos-rg__item--selected {
  border-color: color-mix(in srgb, var(--accent) 65%, transparent);
  background: color-mix(in srgb, var(--accent) 7%, var(--background));
}

.kosmos-rg__item--disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.kosmos-rg__native {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

.kosmos-rg__dot {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  margin-top: 2px;
  border: 2px solid color-mix(in srgb, var(--foreground) 35%, transparent);
  border-radius: 50%;
  flex-shrink: 0;
  transition: border-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-rg__item--selected .kosmos-rg__dot {
  border-color: var(--accent);
}

.kosmos-rg__dot-inner {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--accent);
  transform: scale(0);
  transition: transform 140ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-rg__item--selected .kosmos-rg__dot-inner {
  transform: scale(1);
}

.kosmos-rg__text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.kosmos-rg__label {
  font-family: var(--font-sans);
  font-size: 13px;
  font-weight: 500;
  line-height: 1.4;
  color: var(--foreground);
}

.kosmos-rg__desc {
  font-family: var(--font-sans);
  font-size: 11px;
  font-weight: 500;
  line-height: 1.4;
  color: var(--muted-foreground);
}
</style>
