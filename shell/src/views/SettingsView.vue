<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { ArrowUpCircle, Loader2 } from "lucide-vue-next";
import type {
  BackendStatus,
  ExportConverterInfo,
  ExportResult,
  InstalledExtensionInfo,
  MarketplaceCatalog,
  MarketplaceExtension,
  UpdateState,
} from "@shared/ipc-types";

type Tab = "general" | "extensions" | "export";

interface ExportHistoryEntry {
  converter_id: string;
  display_name: string;
  format: string;
  dest_dir: string;
  timestamp: number;
  ok: boolean;
  file_count: number;
  bytes: number;
}

const EXPORT_HISTORY_KEY = "kepler-export-history";
const EXPORT_HISTORY_LIMIT = 10;

const tab = ref<Tab>("general");

// --- General ----------------------------------------------------------------

const hotkey = ref<string>("");
const hotkeyError = ref<string>("");
const capturing = ref<boolean>(false);

function startCapture() {
  capturing.value = true;
  hotkeyError.value = "";
}

function cancelCapture() {
  capturing.value = false;
}

function keyEventToAccelerator(e: KeyboardEvent): string | null {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Control");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  const key = e.key;
  if (key === "Control" || key === "Alt" || key === "Shift" || key === "Meta") {
    return null; // ждём не-модификатор
  }
  let main: string;
  if (key === " ") main = "Space";
  else if (key === "Escape") return "ESC_CANCEL";
  else if (key.length === 1) main = key.toUpperCase();
  else main = key;
  parts.push(main);
  return parts.join("+");
}

async function onCaptureKey(e: KeyboardEvent) {
  if (!capturing.value) return;
  e.preventDefault();
  e.stopPropagation();
  const acc = keyEventToAccelerator(e);
  if (!acc) return;
  if (acc === "ESC_CANCEL") {
    capturing.value = false;
    return;
  }
  const r = await window.kepler.settings.hotkeySet(acc);
  if (r.ok) {
    hotkey.value = acc;
    hotkeyError.value = "";
  } else {
    hotkeyError.value = `Не удалось зарегистрировать (${r.error ?? "unknown"})`;
  }
  capturing.value = false;
}

async function resetHotkey() {
  const v = await window.kepler.settings.hotkeyReset();
  hotkey.value = v;
  hotkeyError.value = "";
}
const version = ref<string>("");
const autostart = ref<boolean>(false);
const developerMode = ref<boolean>(false);
const usageTracker = ref<boolean>(true);
const backend = ref<BackendStatus>({ running: false, lockFilePath: "" });
const loading = ref<boolean>(true);
const autostartError = ref<string>("");

async function loadGeneral() {
  loading.value = true;
  try {
    const [h, v, a, d, u, b] = await Promise.all([
      window.kepler.settings.hotkey(),
      window.kepler.settings.version(),
      window.kepler.settings.autostart.get(),
      window.kepler.settings.developerMode.get(),
      window.kepler.settings.usageTracker.get(),
      window.kepler.backend.status(),
    ]);
    hotkey.value = h;
    version.value = v;
    autostart.value = a;
    developerMode.value = d;
    usageTracker.value = u;
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

async function onToggleUsageTracker(e: Event) {
  const target = e.target as HTMLInputElement;
  const desired = target.checked;
  try {
    await window.kepler.settings.usageTracker.set(desired);
    usageTracker.value = await window.kepler.settings.usageTracker.get();
  } catch (err) {
    console.warn("usageTracker set failed", err);
    usageTracker.value = await window.kepler.settings.usageTracker.get();
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

// --- Export (Phase 7) -------------------------------------------------------
// Универсальный per-type export. Список конвертеров приходит из
// kepler-backend через `window.kepler.export.list()`. UI ничего не знает
// о конкретных object_type'ах — просто показывает что зарегистрировано.

const exportConverters = ref<ExportConverterInfo[]>([]);
const exportLoading = ref<boolean>(false);
const exportError = ref<string>("");
const exportSelectedFormat = ref<Record<string, string>>({});
const exportBusyId = ref<string>("");
const exportStatusByConverter = ref<Record<string, string>>({});
const exportHistory = ref<ExportHistoryEntry[]>([]);

function loadExportHistory(): ExportHistoryEntry[] {
  try {
    const raw = localStorage.getItem(EXPORT_HISTORY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.slice(0, EXPORT_HISTORY_LIMIT);
  } catch {
    return [];
  }
}

function pushExportHistory(entry: ExportHistoryEntry) {
  const next = [entry, ...exportHistory.value].slice(0, EXPORT_HISTORY_LIMIT);
  exportHistory.value = next;
  try {
    localStorage.setItem(EXPORT_HISTORY_KEY, JSON.stringify(next));
  } catch {
    // ignore quota / unavailable
  }
}

async function loadExportConverters() {
  exportLoading.value = true;
  exportError.value = "";
  try {
    const list = await window.kepler.export.list();
    exportConverters.value = list;
    const sel = { ...exportSelectedFormat.value };
    for (const c of list) {
      if (!sel[c.converter_id]) {
        sel[c.converter_id] = c.default_format;
      }
    }
    exportSelectedFormat.value = sel;
  } catch (e) {
    exportError.value = (e as Error).message;
    exportConverters.value = [];
  } finally {
    exportLoading.value = false;
  }
}

async function onRunExport(c: ExportConverterInfo) {
  if (exportBusyId.value) return;
  const format =
    exportSelectedFormat.value[c.converter_id] ?? c.default_format;
  let destDir: string | null = null;
  try {
    destDir = await window.kepler.export.pickDir();
  } catch (e) {
    exportStatusByConverter.value = {
      ...exportStatusByConverter.value,
      [c.converter_id]: `Ошибка диалога: ${(e as Error).message}`,
    };
    return;
  }
  if (!destDir) return; // отменили

  exportBusyId.value = c.converter_id;
  exportStatusByConverter.value = {
    ...exportStatusByConverter.value,
    [c.converter_id]: "Экспортирую…",
  };
  try {
    const r: ExportResult = await window.kepler.export.run({
      converter_id: c.converter_id,
      format,
      dest_dir: destDir,
    });
    const ok = r.errors.length === 0;
    const sizeKb = (r.bytes / 1024).toFixed(1);
    const parts: string[] = [
      `Готово: ${r.files_written.length} файлов, ${sizeKb} KB`,
    ];
    if (r.errors.length > 0) {
      parts.push(`Ошибок: ${r.errors.length}`);
      const sample = r.errors.slice(0, 3).join("; ");
      parts.push(sample);
    }
    exportStatusByConverter.value = {
      ...exportStatusByConverter.value,
      [c.converter_id]: parts.join(" · "),
    };
    pushExportHistory({
      converter_id: c.converter_id,
      display_name: c.display_name,
      format,
      dest_dir: destDir,
      timestamp: Date.now(),
      ok,
      file_count: r.files_written.length,
      bytes: r.bytes,
    });
  } catch (e) {
    exportStatusByConverter.value = {
      ...exportStatusByConverter.value,
      [c.converter_id]: `Ошибка: ${(e as Error).message}`,
    };
  } finally {
    exportBusyId.value = "";
  }
}

function formatHistoryTime(ts: number): string {
  const d = new Date(ts);
  return `${d.toLocaleDateString("ru")} ${d.toLocaleTimeString("ru", {
    hour: "2-digit",
    minute: "2-digit",
  })}`;
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
  if (t === "export") {
    void loadExportConverters();
    exportHistory.value = loadExportHistory();
  }
}

function catalogById(id: string): MarketplaceExtension | undefined {
  return catalog.value?.extensions.find((e) => e.id === id);
}

function hasUpdate(i: InstalledExtensionInfo): boolean {
  const c = catalogById(i.id);
  return !!(c && i.version && c.version !== i.version);
}

async function onUpdate(i: InstalledExtensionInfo) {
  const c = catalogById(i.id);
  if (!c || installingId.value) return;
  installingId.value = i.id;
  marketError.value = "";
  try {
    await window.kepler.extension.installFromUrl(c.downloadUrl, c.sha256);
    await loadExtensions();
  } catch (e) {
    marketError.value = `${i.id}: ${(e as Error).message}`;
  } finally {
    installingId.value = "";
  }
}

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
          <button
            type="button"
            class="tab"
            :class="{ active: tab === 'export' }"
            @click="selectTab('export')"
          >
            Экспорт
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

      <div v-else class="rows kosmos-scroll">
        <div class="row">
          <div class="row-label">
            <div class="label">Глобальный хоткей</div>
            <div class="hint">Показать или скрыть launcher</div>
            <div v-if="hotkeyError" class="error">{{ hotkeyError }}</div>
          </div>
          <div class="hotkey-control">
            <button
              type="button"
              class="hotkey-capture"
              :class="{ capturing }"
              @click="startCapture"
              @keydown="onCaptureKey"
              @blur="cancelCapture"
            >
              <span v-if="capturing">Нажми сочетание…</span>
              <code v-else class="value">{{ hotkey }}</code>
            </button>
            <button type="button" class="btn ghost" @click="resetHotkey">Сброс</button>
          </div>
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
            <div class="label">Трекать активные приложения</div>
            <div class="hint">
              Записывает в ARK какое окно сейчас активно (process + title).
              Password manager'ы и окна с «password» в title исключаются.
              Изменение применится после перезапуска Kepler.
            </div>
          </div>
          <label class="toggle">
            <input
              type="checkbox"
              :checked="usageTracker"
              @change="onToggleUsageTracker"
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

    <!-- Extensions tab — плоский список установленных. Обновления подтягиваются из catalog.json. -->
    <template v-else-if="tab === 'extensions'">
      <div v-if="marketError" class="error-banner">{{ marketError }}</div>
      <div v-if="extensionsError" class="error-banner">{{ extensionsError }}</div>

      <div class="ext-list kosmos-scroll">
        <div v-if="installed.length === 0" class="empty">
          Расширений нет.
        </div>

        <div
          v-for="ext in installed"
          :key="ext.id"
          class="ext-item"
        >
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
              <span v-if="ext.author" class="ext-author">· {{ ext.author }}</span>
              <span v-if="hasUpdate(ext)" class="ext-author">
                · доступно v{{ catalogById(ext.id)?.version }}
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
              v-if="hasUpdate(ext)"
              type="button"
              class="btn"
              :disabled="installingId === ext.id || busyExt === ext.id"
              @click="onUpdate(ext)"
            >
              <template v-if="installingId === ext.id">Обновление…</template>
              <template v-else>Обновить</template>
            </button>
            <button
              v-if="ext.backupCount > 0"
              type="button"
              class="btn ghost"
              :disabled="busyExt === ext.id || installingId === ext.id"
              @click="onRevert(ext.id)"
            >
              Откатить
            </button>
            <button
              type="button"
              class="btn ghost danger"
              :disabled="busyExt === ext.id || installingId === ext.id"
              @click="onUninstall(ext.id)"
            >
              Удалить
            </button>
          </div>
        </div>
      </div>

      <div class="ext-footer">
        <button
          type="button"
          class="btn ghost"
          :disabled="marketLoading"
          @click="loadCatalog(true)"
        >
          {{ marketLoading ? "Проверка…" : "Проверить обновления" }}
        </button>
      </div>
    </template>

    <!-- Export tab — список зарегистрированных converters + history -->
    <template v-else-if="tab === 'export'">
      <div v-if="exportError" class="error-banner">{{ exportError }}</div>

      <div v-if="exportLoading" class="empty">Загрузка…</div>

      <div v-else class="rows kosmos-scroll">
        <div v-if="exportConverters.length === 0" class="empty">
          Нет доступных конвертеров. Backend ещё не зарегистрировал ни одного.
        </div>

        <div
          v-for="c in exportConverters"
          :key="c.converter_id"
          class="row export-row"
        >
          <div class="row-label">
            <div class="label">{{ c.display_name }}</div>
            <div class="hint">
              {{ c.object_type }} → {{ exportSelectedFormat[c.converter_id] ?? c.default_format }}
            </div>
            <div
              v-if="exportStatusByConverter[c.converter_id]"
              class="hint export-status"
            >
              {{ exportStatusByConverter[c.converter_id] }}
            </div>
          </div>
          <div class="row-actions">
            <select
              v-if="c.supported_formats.length > 1"
              v-model="exportSelectedFormat[c.converter_id]"
              class="export-format-select"
              :disabled="exportBusyId === c.converter_id"
            >
              <option
                v-for="f in c.supported_formats"
                :key="f"
                :value="f"
              >
                {{ f }}
              </option>
            </select>
            <button
              type="button"
              class="btn"
              :disabled="exportBusyId === c.converter_id"
              @click="onRunExport(c)"
            >
              {{ exportBusyId === c.converter_id ? "Экспорт…" : "Экспортировать" }}
            </button>
          </div>
        </div>

        <div
          v-if="exportHistory.length > 0"
          class="ext-section-header"
        >
          Последние экспорты
        </div>
        <div
          v-for="(h, i) in exportHistory"
          :key="i"
          class="row export-history-row"
        >
          <div class="row-label">
            <div class="label">
              {{ h.display_name }}
              <span class="hint">({{ h.format }})</span>
            </div>
            <div class="hint">
              {{ formatHistoryTime(h.timestamp) }} ·
              {{ h.file_count }} файлов ·
              {{ (h.bytes / 1024).toFixed(1) }} KB
            </div>
            <code class="value lock">{{ h.dest_dir }}</code>
          </div>
          <div class="row-actions">
            <span
              class="value"
              :class="{ muted: !h.ok }"
            >
              {{ h.ok ? "ok" : "с ошибками" }}
            </span>
          </div>
        </div>
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

.hotkey-control {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.hotkey-capture {
  font: inherit;
  font-size: 12px;
  padding: 4px 10px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: transparent;
  color: var(--foreground);
  cursor: pointer;
  min-width: 110px;
  text-align: center;
}

.hotkey-capture.capturing {
  background: color-mix(in srgb, oklch(0.55 0.15 250) 28%, transparent);
  border-color: oklch(0.55 0.15 250);
  outline: none;
}

.ext-footer {
  display: flex;
  justify-content: flex-start;
  padding: 12px 16px;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
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

/* Export tab */

.export-row .export-status {
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  margin-top: 4px;
}

.export-format-select {
  font: inherit;
  font-size: 11px;
  padding: 4px 8px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: var(--foreground);
  cursor: pointer;
}

.export-history-row {
  opacity: 0.85;
}

.export-history-row .row-label code.value.lock {
  margin-top: 4px;
  max-width: 100%;
}
</style>
