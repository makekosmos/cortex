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
    options: ReadonlyArray<Option<T>>;
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
    :class="['flex gap-2', direction === 'horizontal' ? 'flex-row flex-wrap' : 'flex-col']"
    role="radiogroup"
  >
    <label
      v-for="opt in options"
      :key="String(opt.value)"
      :class="[
        'flex cursor-default items-start gap-2 rounded-lg border-2 border-solid border-[var(--border)] bg-[color-mix(in_srgb,var(--foreground)_3%,var(--background))] px-4 py-2 transition-[border-color,background-color] duration-140 ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)]',
        opt.value === modelValue
          ? 'border-[color-mix(in_srgb,var(--accent)_65%,transparent)] bg-[color-mix(in_srgb,var(--accent)_7%,var(--background))]'
          : '',
        opt.disabled || disabled ? 'cursor-not-allowed opacity-55' : '',
        !opt.disabled && !disabled && opt.value !== modelValue
          ? 'hover:border-[color-mix(in_srgb,var(--accent)_30%,var(--border))]'
          : '',
      ]"
    >
      <input
        type="radio"
        :name="groupName"
        :value="opt.value"
        :checked="opt.value === modelValue"
        :disabled="opt.disabled || disabled"
        class="sr-only"
        @change="pick(opt)"
      />
      <span
        :class="[
          'inline-flex size-4 shrink-0 items-center justify-center rounded-full border-2 border-solid border-[color-mix(in_srgb,var(--foreground)_35%,transparent)] transition-[border-color] duration-140 ease-[cubic-bezier(0.2,0,0,1)]',
          opt.value === modelValue ? 'border-[var(--accent)]' : '',
        ]"
        aria-hidden="true"
      >
        <span
          :class="[
            'size-2 rounded-full bg-[var(--accent)] transition-transform duration-140 ease-[cubic-bezier(0.2,0,0,1)]',
            opt.value === modelValue ? 'scale-100' : 'scale-0',
          ]"
        />
      </span>
      <span class="flex min-w-0 flex-col gap-0">
        <span
          class="font-[var(--font-sans)] text-[13px] font-medium leading-[1.4] text-[var(--foreground)]"
        >
          {{ opt.label }}
        </span>
        <span
          v-if="opt.description"
          class="font-[var(--font-sans)] text-[11px] font-medium leading-[1.4] text-[var(--muted-foreground)]"
        >
          {{ opt.description }}
        </span>
      </span>
    </label>
  </div>
</template>
