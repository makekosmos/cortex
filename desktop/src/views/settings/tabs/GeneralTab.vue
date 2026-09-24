<script setup lang="ts">
// GeneralTab — глобальный хоткей shell и tray icon.

import { Button, HotkeyCapture } from "@kosmos/visuals";
import LegacyRow from "../components/LegacyRow.vue";
import LegacyToggle from "../components/LegacyToggle.vue";

defineProps<{
  loading: boolean;
  hotkey: string;
  hotkeyError: string;
  trayIcon: boolean;
}>();

defineEmits<{
  hotkeyChange: [v: string];
  resetHotkey: [];
  toggleTrayIcon: [e: Event];
}>();
</script>

<template>
  <div v-if="loading" class="empty">Загрузка…</div>

  <div v-else class="rows kosmos-scroll">
    <LegacyRow title="Глобальный хоткей" hint="Открыть Kosmos" :error="hotkeyError">
      <div class="hotkey-control">
        <HotkeyCapture
          :model-value="hotkey"
          capture-prompt="Нажми сочетание…"
          @update:modelValue="(v: string) => $emit('hotkeyChange', v)"
        />
        <Button variant="ghost" size="sm" @click="$emit('resetHotkey')">Сброс</Button>
      </div>
    </LegacyRow>

    <LegacyRow title="Показывать в трее" hint="Оставлять значок Kosmos в системном трее">
      <LegacyToggle :checked="trayIcon" @change="(e: Event) => $emit('toggleTrayIcon', e)" />
    </LegacyRow>
  </div>
</template>
