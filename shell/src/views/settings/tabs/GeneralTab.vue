<script setup lang="ts">
// GeneralTab — глобальный хоткей лаунчера, autostart, tray icon.

import { Button, HotkeyCapture } from "@kosmos/visuals";
import LegacyRow from "../components/LegacyRow.vue";
import LegacyToggle from "../components/LegacyToggle.vue";

defineProps<{
  loading: boolean;
  hotkey: string;
  hotkeyError: string;
  autostart: boolean;
  autostartAllowed: boolean;
  autostartError: string;
  trayIcon: boolean;
}>();

defineEmits<{
  launcherHotkeyChange: [v: string];
  resetHotkey: [];
  toggleAutostart: [e: Event];
  toggleTrayIcon: [e: Event];
}>();
</script>

<template>
  <div v-if="loading" class="empty">Загрузка…</div>

  <div v-else class="rows kosmos-scroll">
    <LegacyRow
      title="Глобальный хоткей"
      hint="Показать или скрыть launcher"
      :error="hotkeyError"
    >
      <div class="hotkey-control">
        <HotkeyCapture
          :model-value="hotkey"
          capture-prompt="Нажми сочетание…"
          @update:modelValue="(v: string) => $emit('launcherHotkeyChange', v)"
        />
        <Button variant="ghost" size="sm" @click="$emit('resetHotkey')">Сброс</Button>
      </div>
    </LegacyRow>

    <LegacyRow title="Автозапуск с Windows" :error="autostartError">
      <template #hint>
        <template v-if="autostartAllowed">Запускать Kepler при входе в систему</template>
        <template v-else>Доступно только в установленной версии (не в dev-сборке)</template>
      </template>
      <LegacyToggle
        :checked="autostart"
        :disabled="!autostartAllowed"
        @change="(e: Event) => $emit('toggleAutostart', e)"
      />
    </LegacyRow>

    <LegacyRow title="Показывать в трее" hint="Оставлять значок Kepler в системном трее">
      <LegacyToggle :checked="trayIcon" @change="(e: Event) => $emit('toggleTrayIcon', e)" />
    </LegacyRow>
  </div>
</template>
