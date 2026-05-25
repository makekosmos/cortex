<script setup lang="ts">
// DebugTab — Developer mode, launcher TTL, backend info, crash reports, bug bundle.

import LegacyRow from "../components/LegacyRow.vue";
import LegacyToggle from "../components/LegacyToggle.vue";
import type { BackendStatus } from "@shared/ipc-types";

interface CrashFile {
  name: string;
  size: number;
  mtime: string;
}

defineProps<{
  developerMode: boolean;
  launcherStateTtl: number;
  backend: BackendStatus;
  crashFiles: CrashFile[];
  crashesError: string;
  bundleSavedPath: string;
  bundling: boolean;
  bundleError: string;
}>();

defineEmits<{
  toggleDeveloperMode: [e: Event];
  launcherStateTtlChange: [e: Event];
  openCrashesFolder: [];
  clearCrashes: [];
  bundleSave: [];
  openLogsFolder: [];
}>();
</script>

<template>
  <div class="rows kosmos-scroll">
    <LegacyRow
      title="Developer mode"
      hint="Hot reload extension'ов через Vite + F12 для DevTools. Перезапусти extension чтобы применить."
    >
      <LegacyToggle
        :checked="developerMode"
        @change="(e: Event) => $emit('toggleDeveloperMode', e)"
      />
    </LegacyRow>

    <LegacyRow title="Запоминать позицию в лаунчере">
      <template #hint>
        Сохраняет введённый текст, выбранный пункт и прокрутку между открытиями лаунчера.
        <code>0</code> — всегда ресетить при открытии.
      </template>
      <label class="ttl-input">
        <input
          type="number"
          min="0"
          max="1440"
          step="1"
          :value="launcherStateTtl"
          @change="(e: Event) => $emit('launcherStateTtlChange', e)"
        />
        <span>мин</span>
      </label>
    </LegacyRow>

    <LegacyRow title="Backend" hint="kepler-backend подпроцесс">
      <code v-if="backend.running" class="value">
        pid {{ backend.pid }} • port {{ backend.wsPort }}
      </code>
      <span v-else class="value muted">не запущен</span>
    </LegacyRow>

    <LegacyRow v-if="backend.lockFilePath" title="Lock-файл">
      <code class="value lock">{{ backend.lockFilePath }}</code>
    </LegacyRow>

    <LegacyRow title="Отчёты об ошибках">
      <template #hint>
        <template v-if="crashFiles.length === 0">
          Crash-логов нет. Если Kepler упадёт, файлы появятся здесь.
        </template>
        <template v-else>
          {{ crashFiles.length }} {{ crashFiles.length === 1 ? "файл" : "файла(-ов)" }} в папке
          отчётов.
        </template>
      </template>
      <div class="row-actions">
        <button type="button" class="btn ghost" @click="$emit('openCrashesFolder')">
          Открыть папку
        </button>
        <button
          type="button"
          class="btn ghost"
          :disabled="crashFiles.length === 0"
          @click="$emit('clearCrashes')"
        >
          Очистить
        </button>
      </div>
    </LegacyRow>
    <div v-if="crashesError" class="error-banner">{{ crashesError }}</div>

    <LegacyRow title="Bug-report (ZIP)">
      <template #hint>
        <template v-if="bundleSavedPath">Сохранён: <code>{{ bundleSavedPath }}</code></template>
        <template v-else-if="bundling">Собираю отчёт…</template>
        <template v-else>Логи + crash-reports + версии.</template>
      </template>
      <div class="row-actions">
        <button type="button" class="btn ghost" :disabled="bundling" @click="$emit('bundleSave')">
          Создать отчёт
        </button>
        <button type="button" class="btn ghost" @click="$emit('openLogsFolder')">
          Открыть logs/
        </button>
      </div>
    </LegacyRow>
    <div v-if="bundleError" class="error-banner">{{ bundleError }}</div>
  </div>
</template>
