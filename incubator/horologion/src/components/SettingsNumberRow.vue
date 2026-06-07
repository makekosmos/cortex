<script setup lang="ts">
// SettingsNumberRow — SettingsRow + number input + единица измерения.
// Заменяет повторяющийся 18-строчный inline-паттерн в горологионовских
// настройках (4× одинаковых блока для work/short break/long break/pomodoros).
//
// modelValue: number, обновляется через @change (как у оригинала).
// `clamp` — функция clipping значения, вызывается на change.

import { SettingsRow } from "@kosmos/visuals";

interface Props {
  title: string;
  description?: string;
  modelValue: number;
  min: number;
  max: number;
  unit: string;
  /** Опциональный кастомный clamp (по умолчанию Math.min/max + floor). */
  clamp?: (raw: number) => number;
}

const props = withDefaults(defineProps<Props>(), {
  description: undefined,
  clamp: undefined,
});

const emit = defineEmits<{ "update:modelValue": [v: number] }>();

function onChange(e: Event) {
  const raw = Number((e.target as HTMLInputElement).value);
  const clamped = props.clamp
    ? props.clamp(raw)
    : Math.max(props.min, Math.min(props.max, Math.floor(raw)));
  emit("update:modelValue", clamped);
}
</script>

<template>
  <SettingsRow :title="title" :description="description">
    <template #control>
      <span class="row__control">
        <input type="number" :min="min" :max="max" :value="modelValue" @change="onChange" />
        <span class="row__unit">{{ unit }}</span>
      </span>
    </template>
  </SettingsRow>
</template>

<style scoped>
.row__control {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
}

.row__control input[type="number"] {
  width: 72px;
  height: 34px;
  text-align: right;
  padding: 0 0.625rem;
  background: color-mix(in srgb, var(--foreground) 4%, var(--background));
  border: 2px solid var(--border);
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  color: var(--foreground);
  font-family: var(--font-mono);
  font-size: 0.875rem;
  font-variant-numeric: tabular-nums;
  outline: none;
  transition: border-color 140ms cubic-bezier(0.2, 0, 0, 1);
}

.row__control input[type="number"]:focus {
  border-color: color-mix(in srgb, var(--accent) 65%, transparent);
}

.row__control input[type="number"]::-webkit-inner-spin-button,
.row__control input[type="number"]::-webkit-outer-spin-button {
  -webkit-appearance: none;
  appearance: none;
  margin: 0;
}

.row__control input[type="number"] {
  -moz-appearance: textfield;
  appearance: textfield;
}

.row__unit {
  font-size: 0.8125rem;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  min-width: 28px;
}
</style>
