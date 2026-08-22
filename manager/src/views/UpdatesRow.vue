<script setup lang="ts">
import { Button, SettingsRow } from "@kosmos/visuals";

defineProps<{
  title: string;
  id?: string;
  icon: string;
  current: string;
  available?: string | null;
  status?: string;
  actionLabel?: string;
  disabled?: boolean;
  busy?: boolean;
}>();
defineEmits<{ action: [] }>();
</script>

<template>
  <SettingsRow :title="title" :description="status || current">
    <template #leading-icon>
      <span
        class="store-card-icon-frame"
        :class="id ? `store-card-icon-frame--${id.replaceAll('.', '-')}` : undefined"
      >
        <img class="store-card-icon" :src="icon" alt="" />
      </span>
    </template>
    <template #control>
      <div class="updates-row-control">
        <span v-if="available">Доступна {{ available }}</span>
        <Button
          v-if="actionLabel"
          size="sm"
          :disabled="disabled || busy"
          :loading="busy"
          @click="$emit('action')"
          >{{ busy ? "Обновление…" : actionLabel }}</Button
        >
      </div>
    </template>
  </SettingsRow>
</template>
