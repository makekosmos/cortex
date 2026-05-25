<script setup lang="ts">
// DebugTab — Developer mode, launcher TTL, backend info, crash reports, bug bundle.
// Часть decomposition'а SettingsView.

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
    <div class="row">
      <div class="row-label">
        <div class="label">Developer mode</div>
        <div class="hint">
          Hot reload extension'ов через Vite + F12 для DevTools. Перезапусти extension чтобы
          применить.
        </div>
      </div>
      <LegacyToggle
        :checked="developerMode"
        @change="(e: Event) => $emit('toggleDeveloperMode', e)"
      />
    </div>

    <div class="row">
      <div class="row-label">
        <div class="label">Запоминать позицию в лаунчере</div>
        <div class="hint">
          Сохраняет введённый текст, выбранный пункт и прокрутку между открытиями лаунчера.
          <code>0</code> — всегда ресетить при открытии.
        </div>
      </div>
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
    </div>

    <div class="row">
      <div class="row-label">
        <div class="label">Backend</div>
        <div class="hint">kepler-backend подпроцесс</div>
      </div>
      <code v-if="backend.running" class="value">
        pid {{ backend.pid }} • port {{ backend.wsPort }}
      </code>
      <span v-else class="value muted">не запущен</span>
    </div>

    <div v-if="backend.lockFilePath" class="row">
      <div class="row-label">
        <div class="label">Lock-файл</div>
      </div>
      <code class="value lock">{{ backend.lockFilePath }}</code>
    </div>

    <div class="row">
      <div class="row-label">
        <div class="label">Отчёты об ошибках</div>
        <div class="hint">
          <template v-if="crashFiles.length === 0">
            Crash-логов нет. Если Kepler упадёт, файлы появятся здесь.
          </template>
          <template v-else>
            {{ crashFiles.length }} {{ crashFiles.length === 1 ? "файл" : "файла(-ов)" }} в папке
            отчётов.
          </template>
        </div>
      </div>
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
    </div>
    <div v-if="crashesError" class="error-banner">{{ crashesError }}</div>

    <div class="row">
      <div class="row-label">
        <div class="label">Bug-report (ZIP)</div>
        <div class="hint">
          <template v-if="bundleSavedPath"> Сохранён: <code>{{ bundleSavedPath }}</code> </template>
          <template v-else-if="bundling">Собираю отчёт…</template>
          <template v-else>Логи + crash-reports + версии.</template>
        </div>
      </div>
      <div class="row-actions">
        <button
          type="button"
          class="btn ghost"
          :disabled="bundling"
          @click="$emit('bundleSave')"
        >
          Создать отчёт
        </button>
        <button type="button" class="btn ghost" @click="$emit('openLogsFolder')">
          Открыть logs/
        </button>
      </div>
    </div>
    <div v-if="bundleError" class="error-banner">{{ bundleError }}</div>
  </div>
</template>
