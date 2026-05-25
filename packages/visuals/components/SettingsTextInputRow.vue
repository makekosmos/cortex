<script setup lang="ts">
// SettingsTextInputRow — атомарная композиция SettingsRow + TextInput.
// Замещает паттерн `<SettingsRow ...><template #control><TextInput ...
// /></template></SettingsRow>` (≥ 2 раз в shell/SettingsView).
//
// Прокидывает наружу @blur (нужно для on-blur нормализации URL / socks).

import SettingsRow from "./SettingsRow.vue";
import TextInput from "./TextInput.vue";

interface Props {
  title: string;
  description?: string;
  modelValue: string;
  placeholder?: string;
  type?: "text" | "password" | "email" | "search" | "url";
  autocomplete?: string;
  disabled?: boolean;
  muted?: boolean;
}

withDefaults(defineProps<Props>(), {
  description: undefined,
  placeholder: undefined,
  type: "text",
  autocomplete: undefined,
  disabled: false,
  muted: false,
});

const emit = defineEmits<{
  "update:modelValue": [v: string];
  blur: [e: FocusEvent];
}>();
</script>

<template>
  <SettingsRow :title="title" :description="description" :muted="muted">
    <template #control>
      <TextInput
        :model-value="modelValue"
        :placeholder="placeholder"
        :type="type"
        :autocomplete="autocomplete"
        :disabled="disabled"
        @update:model-value="(v: string) => emit('update:modelValue', v)"
        @blur="(e: FocusEvent) => emit('blur', e)"
      />
    </template>
  </SettingsRow>
</template>
