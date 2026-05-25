<script setup lang="ts">
// SettingsToggleRow — атомарная композиция SettingsRow + Toggle.
// Заменяет паттерн <SettingsRow><template #control><Toggle .../></template>
// </SettingsRow>. Сохраняет поведение оригинала: aria-label по умолчанию равен title.

import SettingsRow from "./SettingsRow.vue";
import Toggle from "./Toggle.vue";

interface Props {
  title: string;
  description?: string;
  modelValue: boolean;
  disabled?: boolean;
  muted?: boolean;
  ariaLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  description: undefined,
  disabled: false,
  muted: false,
  ariaLabel: undefined,
});

const emit = defineEmits<{
  "update:modelValue": [v: boolean];
}>();
</script>

<template>
  <SettingsRow :title="title" :description="description" :muted="muted">
    <template #control>
      <Toggle
        :model-value="modelValue"
        :disabled="disabled"
        :aria-label="ariaLabel ?? title"
        @update:model-value="(v: boolean) => emit('update:modelValue', v)"
      />
    </template>
  </SettingsRow>
</template>
