<script setup lang="ts">
import { onMounted, ref } from "vue";
import type { BackendStatus } from "@shared/ipc-types";

const hotkey = ref<string>("");
const version = ref<string>("");
const autostart = ref<boolean>(false);
const developerMode = ref<boolean>(false);
const backend = ref<BackendStatus>({ running: false, lockFilePath: "" });
const loading = ref<boolean>(true);
const autostartError = ref<string>("");

async function load() {
  loading.value = true;
  try {
    const [h, v, a, d, b] = await Promise.all([
      window.kepler.settings.hotkey(),
      window.kepler.settings.version(),
      window.kepler.settings.autostart.get(),
      window.kepler.settings.developerMode.get(),
      window.kepler.backend.status(),
    ]);
    hotkey.value = h;
    version.value = v;
    autostart.value = a;
    developerMode.value = d;
    backend.value = b;
  } catch (e) {
    console.warn("settings load failed", e);
  } finally {
    loading.value = false;
  }
}

async function onToggleAutostart(e: Event) {
  const target = e.target as HTMLInputElement;
  const desired = target.checked;
  autostartError.value = "";
  try {
    await window.kepler.settings.autostart.set(desired);
    autostart.value = await window.kepler.settings.autostart.get();
    if (autostart.value !== desired) {
      autostartError.value = "Не удалось применить настройку";
    }
  } catch (err) {
    console.warn("autostart set failed", err);
    autostartError.value = "Ошибка записи в реестр";
    autostart.value = await window.kepler.settings.autostart.get();
  }
}

async function onToggleDeveloperMode(e: Event) {
  const target = e.target as HTMLInputElement;
  const desired = target.checked;
  try {
    await window.kepler.settings.developerMode.set(desired);
    developerMode.value = await window.kepler.settings.developerMode.get();
  } catch (err) {
    console.warn("developerMode set failed", err);
    developerMode.value = await window.kepler.settings.developerMode.get();
  }
}

function onClose() {
  void window.kepler.settings.close();
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    onClose();
  }
}

onMounted(() => {
  void load();
});
</script>

<template>
  <div class="settings" tabindex="0" @keydown="onKey">
    <header class="header">
      <h1>Настройки</h1>
      <button class="close" type="button" @click="onClose" aria-label="Закрыть">
        ×
      </button>
    </header>

    <div v-if="loading" class="empty">Загрузка…</div>

    <div v-else class="rows">
      <div class="row">
        <div class="row-label">
          <div class="label">Глобальный хоткей</div>
          <div class="hint">Показать или скрыть launcher</div>
        </div>
        <code class="value">{{ hotkey }}</code>
      </div>

      <div class="row">
        <div class="row-label">
          <div class="label">Автозапуск с Windows</div>
          <div class="hint">
            Запускать Kepler при входе в систему
          </div>
          <div v-if="autostartError" class="error">{{ autostartError }}</div>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            :checked="autostart"
            @change="onToggleAutostart"
          />
          <span class="track"><span class="thumb" /></span>
        </label>
      </div>

      <div class="row">
        <div class="row-label">
          <div class="label">Developer mode</div>
          <div class="hint">
            Hot reload extension'ов через Vite + F12 для DevTools.
            Перезапусти extension чтобы применить.
          </div>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            :checked="developerMode"
            @change="onToggleDeveloperMode"
          />
          <span class="track"><span class="thumb" /></span>
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

      <div class="row" v-if="backend.lockFilePath">
        <div class="row-label">
          <div class="label">Lock-файл</div>
        </div>
        <code class="value lock">{{ backend.lockFilePath }}</code>
      </div>

      <div class="row">
        <div class="row-label">
          <div class="label">Версия Kepler</div>
        </div>
        <code class="value">{{ version }}</code>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: color-mix(in srgb, oklch(0.04 0 0) 75%, transparent);
  outline: none;
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 22px;
  border-bottom: 1px solid
    color-mix(in srgb, var(--foreground) 8%, transparent);
  -webkit-app-region: drag;
}

.header h1 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: var(--foreground);
}

.close {
  -webkit-app-region: no-drag;
  background: transparent;
  border: none;
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  font-size: 22px;
  line-height: 1;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close:hover {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
  color: var(--foreground);
}

.rows {
  flex: 1;
  overflow-y: auto;
  padding: 8px 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  padding: 12px 10px;
  border-radius: 8px;
}

.row:hover {
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
}

.row-label {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.label {
  color: var(--foreground);
  font-size: 13px;
  font-weight: 500;
}

.hint {
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
  font-size: 11px;
}

.error {
  color: oklch(0.65 0.22 25);
  font-size: 11px;
  margin-top: 2px;
}

.value {
  font-size: 12px;
  font-family: var(--font-mono, ui-monospace, monospace);
  color: color-mix(in srgb, var(--foreground) 80%, transparent);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  padding: 4px 8px;
  border-radius: 6px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 280px;
}

.value.lock {
  font-size: 11px;
  max-width: 320px;
  direction: rtl;
  text-align: left;
}

.value.muted {
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 40%, transparent);
}

.empty {
  padding: 32px 22px;
  text-align: center;
  color: color-mix(in srgb, var(--foreground) 40%, transparent);
  font-size: 13px;
}

.toggle {
  position: relative;
  display: inline-block;
  cursor: pointer;
  flex-shrink: 0;
}

.toggle input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
  width: 0;
  height: 0;
}

.track {
  display: block;
  width: 36px;
  height: 20px;
  border-radius: 12px;
  background: color-mix(in srgb, var(--foreground) 16%, transparent);
  position: relative;
  transition: background 0.15s ease;
}

.thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--foreground);
  transition: transform 0.15s ease;
}

.toggle input:checked + .track {
  background: var(--accent, oklch(0.7 0.18 250));
}

.toggle input:checked + .track .thumb {
  transform: translateX(16px);
}
</style>
