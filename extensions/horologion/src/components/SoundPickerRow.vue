<script setup lang="ts" generic="V extends string">
// SoundPickerRow — SettingsRow + Dropdown с preview-кнопкой.
// Заменяет 2× идентичных inline-паттерна в horologion/SettingsView.vue
// (work-end sound + break-end sound).

import { Play } from "@lucide/vue";
import { Dropdown, SettingsRow } from "@kosmos/visuals";

interface SoundOption<T> {
  value: T;
  label: string;
}

interface Props<T> {
  title: string;
  modelValue: T;
  options: SoundOption<T>[];
}

const props = defineProps<Props<V>>();
const emit = defineEmits<{
  "update:modelValue": [v: V];
  preview: [v: V];
}>();

function onPreview() {
  emit("preview", props.modelValue);
}
</script>

<template>
  <SettingsRow :title="title">
    <template #control>
      <span class="row__control">
        <Dropdown
          :model-value="modelValue"
          :options="options"
          class="dd"
          @update:model-value="(v: V) => emit('update:modelValue', v)"
        />
        <button type="button" class="iconbtn" title="Прослушать" @click="onPreview">
          <Play :size="12" :stroke-width="2" />
        </button>
      </span>
    </template>
  </SettingsRow>
</template>

<style scoped>
/* CSS перенесён из родительского horologion/SettingsView.vue scoped-style:
   .row__control / .dd / .iconbtn (для inner-элементов компонента, которые
   не получают data-v-PARENT). Родитель сохраняет свои копии этих правил
   для остальных контролов (volume-slider).  */
.row__control {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
}

.dd {
  min-width: 160px;
}

.iconbtn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border: 2px solid var(--border);
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  border-radius: calc(var(--radius) * 0.6);
  corner-shape: var(--corner-shape);
  cursor: pointer;
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    border-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.iconbtn:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}
</style>
