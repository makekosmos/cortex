<script setup lang="ts">
// SettingsButtonRow — атомарная композиция SettingsRow + Button.
// Заменяет паттерн <SettingsRow><template #control><Button .../></template>
// </SettingsRow>. Событие @click пробрасывается наружу.

import SettingsRow from "./SettingsRow.vue";
import Button from "./Button.vue";

interface Props {
  title: string;
  description?: string;
  buttonLabel: string;
  variant?: "primary" | "ghost" | "danger";
  size?: "md" | "sm";
  disabled?: boolean;
  loading?: boolean;
  muted?: boolean;
}

withDefaults(defineProps<Props>(), {
  description: undefined,
  variant: "ghost",
  size: "sm",
  disabled: false,
  loading: false,
  muted: false,
});

defineEmits<{ click: [e: MouseEvent] }>();
</script>

<template>
  <SettingsRow :title="title" :description="description" :muted="muted">
    <template #control>
      <Button
        :variant="variant"
        :size="size"
        :disabled="disabled"
        :loading="loading"
        @click="(e: MouseEvent) => $emit('click', e)"
      >
        {{ buttonLabel }}
      </Button>
    </template>
  </SettingsRow>
</template>
