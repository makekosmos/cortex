<script setup lang="ts">
// GeneralTab — глобальный хоткей лаунчера, autostart, tray icon.

import { Button, HotkeyCapture } from "@kosmos/visuals";
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
    <div class="row">
      <div class="row-label">
        <div class="label">Глобальный хоткей</div>
        <div class="hint">Показать или скрыть launcher</div>
        <div v-if="hotkeyError" class="error">{{ hotkeyError }}</div>
      </div>
      <div class="hotkey-control">
        <HotkeyCapture
          :model-value="hotkey"
          capture-prompt="Нажми сочетание…"
          @update:modelValue="(v: string) => $emit('launcherHotkeyChange', v)"
        />
        <Button variant="ghost" size="sm" @click="$emit('resetHotkey')">Сброс</Button>
      </div>
    </div>

    <div class="row">
      <div class="row-label">
        <div class="label">Автозапуск с Windows</div>
        <div class="hint">
          <template v-if="autostartAllowed">Запускать Kepler при входе в систему</template>
          <template v-else>Доступно только в установленной версии (не в dev-сборке)</template>
        </div>
        <div v-if="autostartError" class="error">{{ autostartError }}</div>
      </div>
      <LegacyToggle
        :checked="autostart"
        :disabled="!autostartAllowed"
        @change="(e: Event) => $emit('toggleAutostart', e)"
      />
    </div>

    <div class="row">
      <div class="row-label">
        <div class="label">Показывать в трее</div>
        <div class="hint">Оставлять значок Kepler в системном трее</div>
      </div>
      <LegacyToggle
        :checked="trayIcon"
        @change="(e: Event) => $emit('toggleTrayIcon', e)"
      />
    </div>
  </div>
</template>
