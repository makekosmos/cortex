<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { ArrowUpCircle, Loader2 } from "lucide-vue-next";
import type {
  BackendStatus,
  InstalledExtensionInfo,
  MarketplaceCatalog,
  MarketplaceExtension,
  UpdateState,
} from "@shared/ipc-types";

type Tab = "general" | "extensions";

const tab = ref<Tab>("general");

// --- General ----------------------------------------------------------------

const hotkey = ref<string>("");
const version = ref<string>("");
const autostart = ref<boolean>(false);
const developerMode = ref<boolean>(false);
const backend = ref<BackendStatus>({ running: false, lockFilePath: "" });
const loading = ref<boolean>(true);
const autostartError = ref<string>("");

async function loadGeneral() {
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

// --- Extensions -------------------------------------------------------------

const installed = ref<InstalledExtensionInfo[]>([]);
const extensionsLoading = ref<boolean>(false);
const extensionsError = ref<string>("");
const busyExt = ref<string>("");

async function loadExtensions() {
  extensionsLoading.value = true;
  extensionsError.value = "";
  try {
    installed.value = await window.kepler.extension.installedList();
  } catch (e) {
    extensionsError.value = (e as Error).message;
  } finally {
    extensionsLoading.value = false;
  }
}

async function onRevert(id: string) {
  if (busyExt.value) return;
  busyExt.value = id;
  extensionsError.value = "";
  try {
    const ok = await window.kepler.extension.revert(id);
    if (!ok) {
      extensionsError.value = `${id}: нет доступных backup'ов для отката`;
    }
    await loadExtensions();
  } catch (e) {
    extensionsError.value = `${id}: ${(e as Error).message}`;
  } finally {
    busyExt.value = "";
  }
}

async function onUninstall(id: string) {
  if (busyExt.value) return;
  busyExt.value = id;
  extensionsError.value = "";
  try {
    await window.kepler.extension.uninstall(id);
    await loadExtensions();
  } catch (e) {
    extensionsError.value = `${id}: ${(e as Error).message}`;
  } finally {
    busyExt.value = "";
  }
}

// --- Marketplace ------------------------------------------------------------

const catalog = ref<MarketplaceCatalog | null>(null);
const marketLoading = ref<boolean>(false);
const marketError = ref<string>("");
const installingId = ref<string>("");

async function loadCatalog(force = false) {
  marketLoading.value = true;
  marketError.value = "";
  try {
    catalog.value = await window.kepler.extension.catalogFetch(force);
  } catch (e) {
    marketError.value = (e as Error).message;
  } finally {
    marketLoading.value = false;
  }
}

function installedById(id: string): InstalledExtensionInfo | undefined {
  return installed.value.find((x) => x.id === id);
}

function marketState(ext: MarketplaceExtension): "install" | "update" | "installed" {
  const cur = installedById(ext.id);
  if (!cur) return "install";
  if (cur.version && cur.version !== ext.version) return "update";
  return "installed";
}

async function onMarketInstall(ext: MarketplaceExtension) {
  if (installingId.value) return;
  installingId.value = ext.id;
  marketError.value = "";
  try {
    await window.kepler.extension.installFromUrl(ext.downloadUrl, ext.sha256);
    await loadExtensions();
  } catch (e) {
    marketError.value = `${ext.id}: ${(e as Error).message}`;
  } finally {
    installingId.value = "";
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

function selectTab(t: Tab) {
  tab.value = t;
  if (t === "extensions") {
    void loadExtensions();
    if (!catalog.value) void loadCatalog();
  }
}

/** Установленные что НЕ в catalog (3rd-party / legacy / dev). */
const orphanInstalled = computed<InstalledExtensionInfo[]>(() => {
  if (!catalog.value) return installed.value;
  const catalogIds = new Set(catalog.value.extensions.map((e) => e.id));
  return installed.value.filter((i) => !catalogIds.has(i.id));
});

// --- autoUpdater state ------------------------------------------------------

const updateState = ref<UpdateState>({ kind: "idle" });
const updateChecking = ref<boolean>(false);
let unsubscribeUpdateState: (() => void) | null = null;

async function refreshUpdateState() {
  try {
    updateState.value = await window.kepler.settings.update.state();
  } catch (e) {
    console.warn("update state fetch failed", e);
  }
}

async function onCheckUpdates() {
  if (updateChecking.value) return;
  updateChecking.value = true;
  try {
    updateState.value = await window.kepler.settings.update.check();
  } catch (e) {
    console.warn("update check failed", e);
  } finally {
    updateChecking.value = false;
  }
}

async function onInstallUpdate() {
  try {
    await window.kepler.settings.update.install();
  } catch (e) {
    console.warn("update install failed", e);
  }
}

const updateBanner = computed<null | {
  text: string;
  clickable: boolean;
  progress?: number;
}>(() => {
  const s = updateState.value;
  if (s.kind === "available") {
    return { text: `Доступно обновление Kepler ${s.version}`, clickable: false };
  }
  if (s.kind === "downloading") {
    return {
      text: `Скачивание Kepler ${s.version} (${s.percent}%)`,
      clickable: false,
      progress: s.percent,
    };
  }
  if (s.kind === "downloaded") {
    return {
      text: `Обновление Kepler ${s.version} готово — нажмите чтобы перезапустить`,
      clickable: true,
    };
  }
  return null;
});

const checkResultLabel = computed<string>(() => {
  const s = updateState.value;
  if (s.kind === "checking") return "Проверяем…";
  if (s.kind === "not-available") {
    return `Последняя версия (проверено ${new Date(s.checkedAt).toLocaleTimeString("ru")})`;
  }
  if (s.kind === "error") return `Ошибка: ${s.message}`;
  return "";
});

onMounted(() => {
  void loadGeneral();
  void refreshUpdateState();
  unsubscribeUpdateState = window.kepler.settings.update.onStateChanged((s) => {
    updateState.value = s;
  });
});

onBeforeUnmount(() => {
  unsubscribeUpdateState?.();
  unsubscribeUpdateState = null;
});
</script>

<template>
  <div class="settings" tabindex="0" @keydown="onKey">
    <!-- Raycast-style update banner. Шириной во всё окно, height ~32px. -->
    <button
      v-if="updateBanner"
      type="button"
      class="update-banner"
      :class="{ clickable: updateBanner.clickable }"
      :disabled="!updateBanner.clickable"
      @click="updateBanner.clickable && onInstallUpdate()"
    >
      <component
        :is="updateState.kind === 'downloading' ? Loader2 : ArrowUpCircle"
        :size="14"
        :class="{ spin: updateState.kind === 'downloading' }"
      />
      <span class="update-banner-text">{{ updateBanner.text }}</span>
      <span
        v-if="updateBanner.progress !== undefined"
        class="update-banner-progress"
        :style="{ width: `${updateBanner.progress}%` }"
      />
    </button>

    <header class="header">
      <div class="header-left">
        <h1>Настройки</h1>
        <nav class="tabs">
          <button
            type="button"
            class="tab"
            :class="{ active: tab === 'general' }"
            @click="selectTab('general')"
          >
            Общие
          </button>
          <button
            type="button"
            class="tab"
            :class="{ active: tab === 'extensions' }"
            @click="selectTab('extensions')"
          >
            Расширения
          </button>
        </nav>
      </div>
      <button class="close" type="button" @click="onClose" aria-label="Закрыть">
        ×
      </button>
    </header>

    <!-- General tab -->
    <template v-if="tab === 'general'">
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
            <div class="hint">Запускать Kepler при входе в систему</div>
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
            <div v-if="checkResultLabel" class="hint">{{ checkResultLabel }}</div>
          </div>
          <div class="row-actions">
            <code class="value">{{ version }}</code>
            <button
              type="button"
              class="btn ghost"
              :disabled="updateChecking || updateState.kind === 'downloading'"
              @click="onCheckUpdates"
            >
              {{ updateChecking ? "Проверяем…" : "Проверить обновления" }}
            </button>
          </div>
        </div>
      </div>
    </template>

    <!-- Extensions tab — единый список из catalog.json + orphan installed внизу -->
    <template v-else>
      <div class="market-header">
        <span class="hint" v-if="catalog">
          Каталог обновлён: {{ new Date(catalog.updatedAt).toLocaleString("ru") }}
        </span>
        <span class="hint" v-else-if="marketLoading">Загрузка каталога…</span>
        <button
          type="button"
          class="btn ghost"
          :disabled="marketLoading"
          @click="loadCatalog(true)"
        >
          Обновить
        </button>
      </div>

      <div v-if="marketError" class="error-banner">{{ marketError }}</div>
      <div v-if="extensionsError" class="error-banner">{{ extensionsError }}</div>

      <div class="ext-list">
        <div
          v-if="!marketLoading && catalog && catalog.extensions.length === 0"
          class="empty"
        >
          Каталог пуст.
        </div>

        <div
          v-for="ext in catalog?.extensions ?? []"
          :key="ext.id"
          class="ext-item"
        >
          <img
            v-if="ext.iconUrl"
            class="ext-icon"
            :src="ext.iconUrl"
            alt=""
            @error="($event.target as HTMLImageElement).style.display = 'none'"
          />
          <div v-else class="ext-icon ext-icon-fallback">
            {{ ext.name.slice(0, 1) }}
          </div>
          <div class="ext-info">
            <div class="ext-name">{{ ext.name }}</div>
            <div class="ext-meta">
              <span class="ext-version">v{{ ext.version }}</span>
              <span class="ext-author">· {{ ext.author }}</span>
              <span
                v-if="marketState(ext) === 'update' && installedById(ext.id)"
                class="ext-author"
              >
                · установлено v{{ installedById(ext.id)?.version }}
              </span>
              <span
                v-else-if="marketState(ext) === 'installed'"
                class="ext-author"
              >
                · установлено
              </span>
              <span
                v-if="(installedById(ext.id)?.backupCount ?? 0) > 0"
                class="ext-backups"
              >
                · backup'ов: {{ installedById(ext.id)?.backupCount }}
              </span>
            </div>
            <div v-if="ext.description" class="ext-description">
              {{ ext.description }}
            </div>
          </div>
          <div class="ext-actions">
            <button
              v-if="(installedById(ext.id)?.backupCount ?? 0) > 0"
              type="button"
              class="btn ghost"
              :disabled="busyExt === ext.id || installingId === ext.id"
              @click="onRevert(ext.id)"
            >
              Откатить
            </button>
            <button
              v-if="installedById(ext.id)"
              type="button"
              class="btn ghost danger"
              :disabled="busyExt === ext.id || installingId === ext.id"
              @click="onUninstall(ext.id)"
            >
              Удалить
            </button>
            <button
              type="button"
              class="btn"
              :disabled="
                installingId === ext.id ||
                busyExt === ext.id ||
                marketState(ext) === 'installed'
              "
              @click="onMarketInstall(ext)"
            >
              <template v-if="installingId === ext.id">Установка…</template>
              <template v-else-if="marketState(ext) === 'update'">
                Обновить
              </template>
              <template v-else-if="marketState(ext) === 'installed'">
                Установлено
              </template>
              <template v-else>Установить</template>
            </button>
          </div>
        </div>

        <!-- Orphan installed — есть локально, но нет в catalog (3rd-party / legacy / dev install) -->
        <template v-if="orphanInstalled.length > 0">
          <div class="ext-section-header">Прочие установленные</div>
          <div v-for="ext in orphanInstalled" :key="ext.id" class="ext-item">
            <img
              v-if="ext.iconDataUri"
              class="ext-icon"
              :src="ext.iconDataUri"
              alt=""
            />
            <div v-else class="ext-icon ext-icon-fallback">
              {{ ext.name.slice(0, 1) }}
            </div>
            <div class="ext-info">
              <div class="ext-name">{{ ext.name }}</div>
              <div class="ext-meta">
                <span class="ext-version">v{{ ext.version ?? "—" }}</span>
                <span v-if="ext.author" class="ext-author">
                  · {{ ext.author }}
                </span>
                <span v-if="ext.backupCount > 0" class="ext-backups">
                  · backup'ов: {{ ext.backupCount }}
                </span>
              </div>
              <div v-if="ext.description" class="ext-description">
                {{ ext.description }}
              </div>
            </div>
            <div class="ext-actions">
              <button
                v-if="ext.backupCount > 0"
                type="button"
                class="btn ghost"
                :disabled="busyExt === ext.id"
                @click="onRevert(ext.id)"
              >
                Откатить
              </button>
              <button
                type="button"
                class="btn ghost danger"
                :disabled="busyExt === ext.id"
                @click="onUninstall(ext.id)"
              >
                Удалить
              </button>
            </div>
          </div>
        </template>
      </div>
    </template>
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

/* Raycast-style update banner — высота ~32px, во всю ширину, прижат к
   самому верху над header. Прогресс-бар — нижняя полоска заполняется. */
.update-banner {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
  height: 32px;
  border: none;
  background: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 20%, transparent);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  font-weight: 500;
  cursor: default;
  -webkit-app-region: no-drag;
  overflow: hidden;
}

.update-banner.clickable {
  cursor: pointer;
  background: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 35%, transparent);
}

.update-banner.clickable:hover {
  background: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 50%, transparent);
}

.update-banner-text {
  flex-shrink: 1;
  text-overflow: ellipsis;
  overflow: hidden;
  white-space: nowrap;
}

.update-banner-progress {
  position: absolute;
  bottom: 0;
  left: 0;
  height: 2px;
  background: var(--accent, oklch(0.7 0.18 250));
  transition: width 200ms ease-out;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.row-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding: 16px 22px 10px;
  border-bottom: 1px solid
    color-mix(in srgb, var(--foreground) 8%, transparent);
  -webkit-app-region: drag;
  gap: 16px;
}

.header-left {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
}

.header h1 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: var(--foreground);
}

.tabs {
  display: flex;
  gap: 4px;
  -webkit-app-region: no-drag;
}

.tab {
  font: inherit;
  font-size: 12px;
  padding: 4px 10px;
  border-radius: 6px;
  border: none;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  cursor: pointer;
}

.tab:hover {
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: var(--foreground);
}

.tab.active {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
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

.error,
.error-banner {
  color: oklch(0.65 0.22 25);
  font-size: 11px;
  margin-top: 2px;
}

.error-banner {
  padding: 8px 12px;
  background: color-mix(in srgb, oklch(0.65 0.22 25) 12%, transparent);
  border-radius: 6px;
  font-size: 12px;
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

.empty code {
  font-size: 11px;
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  padding: 2px 6px;
  border-radius: 4px;
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

.market-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px 0;
}

.ext-section-header {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  color: color-mix(in srgb, var(--foreground) 45%, transparent);
  margin: 16px 0 4px;
  padding: 0 2px;
}

/* Extensions list */

.ext-list {
  flex: 1;
  overflow-y: auto;
  padding: 10px 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ext-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
}

.ext-icon {
  width: 40px;
  height: 40px;
  border-radius: 8px;
  object-fit: cover;
  flex-shrink: 0;
}

.ext-icon-fallback {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  font-weight: 600;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
}

.ext-info {
  flex: 1;
  min-width: 0;
}

.ext-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--foreground);
}

.ext-meta {
  font-size: 11px;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  margin-top: 1px;
}

.ext-description {
  font-size: 11px;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  margin-top: 3px;
}

.ext-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.btn {
  font: inherit;
  font-size: 11px;
  padding: 5px 10px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: var(--foreground);
  cursor: pointer;
}

.btn:hover:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 12%, transparent);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn.ghost {
  background: transparent;
}

.btn.danger {
  color: oklch(0.7 0.18 25);
  border-color: color-mix(in srgb, oklch(0.65 0.22 25) 30%, transparent);
}

.btn.danger:hover:not(:disabled) {
  background: color-mix(in srgb, oklch(0.65 0.22 25) 14%, transparent);
}
</style>
