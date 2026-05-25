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
