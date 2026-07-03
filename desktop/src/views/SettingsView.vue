<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  appCommandSettings,
  HIDDEN_COMMANDS_KEY,
  settingsNavigationItems,
  type AISettingsView,
  type AppCommandSetting,
  type AppSettingsTab,
  type SettingsNavigationItem,
  type Tab,
} from "./settings/navigation";
import UpdateBanner from "./settings/components/UpdateBanner.vue";
import AboutTab from "./settings/tabs/AboutTab.vue";
import AppCommandsTab from "./settings/tabs/AppCommandsTab.vue";
import ClipboardSettingsTab from "./settings/tabs/ClipboardSettingsTab.vue";
import DebugTab from "./settings/tabs/DebugTab.vue";
import DictationTab from "./settings/tabs/DictationTab.vue";
import AISettingsTab from "./settings/tabs/AISettingsTab.vue";
import ExportTab from "./settings/tabs/ExportTab.vue";
import ExtensionsTab from "./settings/tabs/ExtensionsTab.vue";
import FileSearchTab from "./settings/tabs/FileSearchTab.vue";
import FocusTab from "./settings/tabs/FocusTab.vue";
import GeneralTab from "./settings/tabs/GeneralTab.vue";
import SecretsTab from "./settings/tabs/SecretsTab.vue";
import SecurityTab from "./settings/tabs/SecurityTab.vue";
import SyncTab from "./settings/tabs/SyncTab.vue";
import {
  createDictationConfig,
  DictationConfigKey,
} from "./settings/composables/useDictationConfig";
import {
  resolveAutostartApplyFailure,
  resolveAutostartApplySuccess,
} from "./settings/autostart-ui";
import { useKeplerUpdate } from "./settings/composables/useKeplerUpdate";
import "@kosmos/visuals/components/settings-shell.css";
import "./settings/settings-shared.css";
import {
  SettingsContentHeader,
  SettingsSearchInput,
  SettingsSidebar,
  SettingsSidebarButton,
  ToastHost,
  provideToastHost,
  usePlatform,
} from "@kosmos/visuals";
import SettingsTopTabs from "./settings/components/SettingsTopTabs.vue";
import type { BackendStatus } from "@shared/ipc-types";

// macOS: настройки в стиле Raycast — горизонтальный таб-бар сверху вместо
// вертикального сайдбара. Те же navigation items, другой layout.
const { isMac } = usePlatform();

// Focus-tab types/constants — `./settings/composables/useFocusTab.ts`.

const tab = ref<Tab>("general");
const searchQuery = ref<string>("");
const aiSettingsView = ref<AISettingsView>("overview");
type SettingsLocation = { tab: Tab; aiView?: AISettingsView };
const backStack = ref<SettingsLocation[]>([]);
const forwardStack = ref<SettingsLocation[]>([]);

function normalizeSearchValue(value: string): string {
  return value.trim().toLowerCase();
}

function matchesNavigationItem(item: SettingsNavigationItem, normalizedQuery: string): boolean {
  if (!normalizedQuery) return true;
  return [item.label, ...item.keywords].some((candidate) =>
    candidate.toLowerCase().includes(normalizedQuery),
  );
}

const normalizedSearchQuery = computed(() => normalizeSearchValue(searchQuery.value));

const matchedNavigationItems = computed(() =>
  settingsNavigationItems.filter((item) =>
    matchesNavigationItem(item, normalizedSearchQuery.value),
  ),
);

const mainNavigationItems = computed(() =>
  matchedNavigationItems.value.filter((item) => item.group === "main"),
);

const advancedNavigationItems = computed(() =>
  matchedNavigationItems.value.filter((item) => item.group === "advanced"),
);

const hasSidebarMatches = computed(() => matchedNavigationItems.value.length > 0);

// Плоский список табов для macOS top-tab bar (main + advanced подряд).
const topTabItems = computed(() => [
  ...mainNavigationItems.value,
  ...advancedNavigationItems.value,
]);

const activeTab = computed<Tab | null>(() => {
  if (!normalizedSearchQuery.value) return tab.value;
  if (matchedNavigationItems.value.some((item) => item.tab === tab.value)) {
    return tab.value;
  }
  return matchedNavigationItems.value[0]?.tab ?? null;
});

const activeNavigationItem = computed(() =>
  settingsNavigationItems.find((item) => item.tab === activeTab.value),
);

const activeLayout = computed<"basic" | "advanced">(
  () => activeNavigationItem.value?.layout ?? "basic",
);

const fileSearchFeatureEnabled = ref<boolean | null>(null);
const featureToggleBusy = ref<boolean>(false);
const fileSearchTabRevision = ref(0);

const activeFeatureToggle = computed(() => {
  if (activeTab.value !== "file-search" || fileSearchFeatureEnabled.value === null) {
    return null;
  }
  return {
    enabled: fileSearchFeatureEnabled.value,
    label: fileSearchFeatureEnabled.value ? "Выключить поиск файлов" : "Включить поиск файлов",
  };
});

const showAdvancedToggle = computed(() => activeFeatureToggle.value !== null);
const canGoBack = computed(() => backStack.value.length > 0);
const canGoForward = computed(() => forwardStack.value.length > 0);

const activeAdvancedIntro = computed(() => {
  const item = activeNavigationItem.value;
  if (!item || (item.layout !== "advanced" && !item.introImage)) return null;
  return item;
});

const hiddenCommandIds = ref<string[]>(loadHiddenCommandIds());

const activeAppCommands = computed<AppCommandSetting[]>(() => {
  const current = activeTab.value;
  if (!current || !isAppSettingsTab(current)) return [];
  return appCommandSettings[current];
});

function isAppSettingsTab(value: Tab): value is AppSettingsTab {
  return (
    value === "notes" ||
    value === "tasks" ||
    value === "time-tracker" ||
    value === "games" ||
    value === "dictation"
  );
}

function loadHiddenCommandIds(): string[] {
  try {
    const raw = localStorage.getItem(HIDDEN_COMMANDS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === "string") : [];
  } catch {
    return [];
  }
}

function saveHiddenCommandIds(ids: string[]) {
  const normalized = Array.from(new Set(ids)).sort();
  localStorage.setItem(HIDDEN_COMMANDS_KEY, JSON.stringify(normalized));
  hiddenCommandIds.value = normalized;
}

function isCommandVisible(id: string): boolean {
  return !hiddenCommandIds.value.includes(id);
}

function onToggleCommandVisibility(id: string, event: Event) {
  const checked = (event.target as HTMLInputElement).checked;
  const next = new Set(hiddenCommandIds.value);
  if (checked) {
    next.delete(id);
  } else {
    next.add(id);
  }
  saveHiddenCommandIds(Array.from(next));
}

// --- General ----------------------------------------------------------------

const hotkey = ref<string>("");
const hotkeyError = ref<string>("");

async function onLauncherHotkeyChange(acc: string) {
  const r = await window.kepler.settings.hotkeySet(acc);
  if (r.ok) {
    hotkey.value = acc;
    hotkeyError.value = "";
  } else {
    hotkeyError.value = `Не удалось зарегистрировать (${r.error ?? "unknown"})`;
  }
}

async function resetHotkey() {
  const v = await window.kepler.settings.hotkeyReset();
  hotkey.value = v;
  hotkeyError.value = "";
}
const version = ref<string>("");
const autostart = ref<boolean>(false);
const autostartAllowed = ref<boolean>(true);
const developerMode = ref<boolean>(false);
const usageTracker = ref<boolean>(true);
const trayIcon = ref<boolean>(true);
const launcherStateTtl = ref<number>(5);
// FileSearch tab — state + handlers инкапсулированы в
// `./settings/composables/useFileSearchTab.ts`. Toast host провайдится здесь
// один раз через `provideToastHost`, так как `<ToastHost />` живёт в template
// SettingsView (overlay общий на все табы).
provideToastHost();

// Dictation config singleton — Security/Secrets/Dictation табы дёргают один
// state через `inject(DictationConfigKey)`. Загрузка вызывается из onMounted'ов
// потребителей; provider создаёт фабрику с пустым default state.
import { provide } from "vue";
provide(DictationConfigKey, createDictationConfig());
const backend = ref<BackendStatus>({ running: false, lockFilePath: "" });
const loading = ref<boolean>(true);
const autostartError = ref<string>("");

async function loadGeneral() {
  loading.value = true;
  try {
    const [h, v, a, aAllowed, d, u, ttl, b] = await Promise.all([
      window.kepler.settings.hotkey(),
      window.kepler.settings.version(),
      window.kepler.settings.autostart.get(),
      window.kepler.settings.autostart.allowed(),
      window.kepler.settings.developerMode.get(),
      window.kepler.settings.usageTracker.get(),
      window.kepler.settings.launcherStateTtl.get(),
      window.kepler.backend.status(),
    ]);
    hotkey.value = h;
    version.value = v;
    autostart.value = a;
    autostartAllowed.value = aAllowed;
    developerMode.value = d;
    usageTracker.value = u;
    launcherStateTtl.value = ttl;
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
    // См. postmortems.md § 2026-06-04. Windows autorun readback can lag
    // behind a successful set, so immediate mismatch is not a user-facing
    // failure. A later loadGeneral/open will reconcile real state.
    const applied = resolveAutostartApplySuccess(desired);
    autostart.value = applied.checked;
    autostartError.value = applied.error;
  } catch (err) {
    console.warn("autostart set failed", err);
    const current = await window.kepler.settings.autostart.get();
    const failed = resolveAutostartApplyFailure(current);
    autostart.value = failed.checked;
    autostartError.value = failed.error;
  }
}

async function loadTrayIcon() {
  try {
    trayIcon.value = await window.kepler.settings.trayIcon.get();
  } catch {
    trayIcon.value = true;
  }
}

async function onToggleTrayIcon(e: Event) {
  const desired = (e.target as HTMLInputElement).checked;
  trayIcon.value = desired;
  try {
    await window.kepler.settings.trayIcon.set(desired);
    trayIcon.value = await window.kepler.settings.trayIcon.get();
  } catch (err) {
    console.warn("trayIcon set failed", err);
    trayIcon.value = await window.kepler.settings.trayIcon.get();
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

async function onLauncherStateTtlChange(e: Event) {
  const target = e.target as HTMLInputElement;
  const minutes = Number(target.value);
  if (!Number.isFinite(minutes) || minutes < 0) return;
  try {
    await window.kepler.settings.launcherStateTtl.set(minutes);
    launcherStateTtl.value = await window.kepler.settings.launcherStateTtl.get();
  } catch (err) {
    console.warn("launcherStateTtl set failed", err);
    launcherStateTtl.value = await window.kepler.settings.launcherStateTtl.get();
  }
}

// инициализация и lifecycle — в `./settings/tabs/FocusTab.vue`.

function onClose() {
  void window.kepler.settings.close();
}

function currentLocation(): SettingsLocation {
  return tab.value === "ai" ? { tab: tab.value, aiView: aiSettingsView.value } : { tab: tab.value };
}

function sameLocation(left: SettingsLocation, right: SettingsLocation): boolean {
  return left.tab === right.tab && (left.aiView ?? "overview") === (right.aiView ?? "overview");
}

function applyLocation(location: SettingsLocation) {
  tab.value = location.tab;
  if (location.tab === "ai") {
    aiSettingsView.value = location.aiView ?? "overview";
  }
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    onClose();
  }
}

// All tab data loading инкапсулировано в их composables (useExportTab,
// useFocusTab, useExtensionsTab, useFileSearchTab, useDictationConfig).
// SettingsView больше не загружает tab data сам.

function navigateToLocation(location: SettingsLocation, recordHistory = true) {
  if (sameLocation(currentLocation(), location)) return;
  if (recordHistory) {
    backStack.value = [...backStack.value, currentLocation()];
    forwardStack.value = [];
  }
  applyLocation(location);
}

function navigateToTab(t: Tab, recordHistory = true) {
  navigateToLocation({ tab: t, aiView: t === "ai" ? "overview" : undefined }, recordHistory);
}

function selectTab(t: Tab) {
  navigateToTab(t);
}

function onAiSettingsViewChange(next: AISettingsView) {
  navigateToLocation({ tab: "ai", aiView: next });
}

function goBack() {
  const previous = backStack.value.at(-1);
  if (!previous) return;
  searchQuery.value = "";
  backStack.value = backStack.value.slice(0, -1);
  forwardStack.value = [...forwardStack.value, currentLocation()];
  applyLocation(previous);
}

function goForward() {
  const next = forwardStack.value.at(-1);
  if (!next) return;
  searchQuery.value = "";
  forwardStack.value = forwardStack.value.slice(0, -1);
  backStack.value = [...backStack.value, currentLocation()];
  applyLocation(next);
}

async function refreshActiveFeatureToggle() {
  if (activeTab.value !== "file-search") {
    return;
  }
  try {
    const settings = await window.kepler.fileSearch.settingsGet();
    fileSearchFeatureEnabled.value = settings.enabled;
  } catch (err) {
    console.warn("file search feature toggle load failed", err);
    fileSearchFeatureEnabled.value = null;
  }
}

async function onToggleActiveFeature() {
  const toggle = activeFeatureToggle.value;
  if (!toggle || featureToggleBusy.value) return;
  featureToggleBusy.value = true;
  const nextEnabled = !toggle.enabled;
  try {
    await window.kepler.fileSearch.settingsSet({ enabled: nextEnabled });
    fileSearchFeatureEnabled.value = nextEnabled;
    fileSearchTabRevision.value += 1;
  } catch (err) {
    console.warn("feature toggle set failed", err);
    await refreshActiveFeatureToggle();
  } finally {
    featureToggleBusy.value = false;
  }
}

watch(activeTab, (next) => {
  if (!searchQuery.value || !next) return;
  if (tab.value !== next) {
    tab.value = next;
  }
});

watch(
  activeTab,
  () => {
    void refreshActiveFeatureToggle();
  },
  { immediate: true },
);

// --- autoUpdater state ------------------------------------------------------

const {
  updateState,
  updateChecking,
  refreshUpdateState,
  onCheckUpdates,
  onInstallUpdate,
  updateBanner,
  checkResultLabel,
} = useKeplerUpdate();
let unsubscribeUpdateState: (() => void) | null = null;

// --- Diagnostics / crashes ------------------------------------------------
const crashFiles = ref<Array<{ name: string; size: number; mtime: string }>>([]);
const crashesError = ref<string>("");

async function refreshCrashes() {
  crashesError.value = "";
  try {
    crashFiles.value = await window.kepler.crashes.list();
  } catch (e) {
    crashesError.value = (e as Error).message;
  }
}

async function onOpenCrashesFolder() {
  try {
    await window.kepler.crashes.openFolder();
  } catch (e) {
    crashesError.value = (e as Error).message;
  }
}

async function onClearCrashes() {
  try {
    await window.kepler.crashes.clear();
    await refreshCrashes();
  } catch (e) {
    crashesError.value = (e as Error).message;
  }
}

// --- Bug bundle (Phase 4 bug-detection) -------------------------------------
const bundling = ref(false);
const bundleSavedPath = ref<string>("");
const bundleError = ref<string>("");
async function onBundleSave() {
  bundleError.value = "";
  bundleSavedPath.value = "";
  bundling.value = true;
  try {
    const saved = await window.kepler.diagnostics.bundleSave();
    if (saved) bundleSavedPath.value = saved;
  } catch (e) {
    bundleError.value = (e as Error).message;
  } finally {
    bundling.value = false;
  }
}
async function onOpenLogsFolder() {
  try {
    await window.kepler.diagnostics.openLogsFolder();
  } catch (e) {
    bundleError.value = (e as Error).message;
  }
}

onMounted(() => {
  void loadGeneral();
  void loadTrayIcon();
  void refreshUpdateState();
  void refreshCrashes();
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
  <div class="settings" :class="{ 'settings--mac': isMac }" tabindex="0" @keydown="onKey">
    <ToastHost />
    <!-- Raycast-style update banner. Шириной во всё окно, height ~32px. -->
    <UpdateBanner :banner="updateBanner" :state="updateState" @install="onInstallUpdate" />

    <!-- macOS: горизонтальный таб-бар (Raycast-style) вместо сайдбара. -->
    <SettingsTopTabs
      v-if="isMac"
      :items="topTabItems"
      :active-tab="activeTab"
      @select="selectTab"
    />

    <div class="settings-shell">
      <SettingsSidebar v-if="!isMac" title="Настройки" background="var(--bg-app, #1d1d1f)">
        <div class="settings-sidebar-search">
          <SettingsSearchInput v-model="searchQuery" placeholder="Поиск" />
        </div>

        <div class="settings-sidebar-scroll kosmos-scroll">
          <template v-if="hasSidebarMatches">
            <div v-if="mainNavigationItems.length > 0" class="settings-sidebar-group">
              <SettingsSidebarButton
                v-for="item in mainNavigationItems"
                :key="item.tab"
                :icon="item.icon"
                :icon-image="item.sidebarImage"
                :icon-from="item.iconGradient?.from"
                :icon-to="item.iconGradient?.to"
                :label="item.label"
                :active="activeTab === item.tab"
                @click="selectTab(item.tab)"
              />
            </div>

            <div v-if="advancedNavigationItems.length > 0" class="settings-sidebar-group">
              <SettingsSidebarButton
                v-for="item in advancedNavigationItems"
                :key="item.tab"
                :icon="item.icon"
                :icon-image="item.sidebarImage"
                :icon-from="item.iconGradient?.from"
                :icon-to="item.iconGradient?.to"
                :label="item.label"
                :active="activeTab === item.tab"
                @click="selectTab(item.tab)"
              />
            </div>
          </template>

          <div v-else class="settings-sidebar-empty">Ничего не найдено</div>
        </div>
      </SettingsSidebar>

      <div class="settings-content">
        <SettingsContentHeader
          v-if="!isMac"
          :back-disabled="!canGoBack"
          :forward-disabled="!canGoForward"
          @back="goBack"
          @forward="goForward"
        >
          <template #right>
            <button
              v-if="showAdvancedToggle"
              type="button"
              class="advanced-feature-toggle"
              :disabled="featureToggleBusy"
              role="switch"
              :aria-checked="String(activeFeatureToggle?.enabled ?? false)"
              :aria-label="activeFeatureToggle?.label ?? 'Переключить функцию'"
              :title="activeFeatureToggle?.label ?? 'Переключить функцию'"
              @click="onToggleActiveFeature"
            >
              <span class="advanced-feature-toggle__thumb" />
            </button>
          </template>
        </SettingsContentHeader>

        <div v-if="searchQuery && !activeTab" class="empty">Ничего не найдено</div>

        <!-- General tab -->
        <template v-else-if="activeTab === 'general'">
          <GeneralTab
            :loading="loading"
            :hotkey="hotkey"
            :hotkey-error="hotkeyError"
            :autostart="autostart"
            :autostart-allowed="autostartAllowed"
            :autostart-error="autostartError"
            :tray-icon="trayIcon"
            @launcher-hotkey-change="onLauncherHotkeyChange"
            @reset-hotkey="resetHotkey"
            @toggle-autostart="onToggleAutostart"
            @toggle-tray-icon="onToggleTrayIcon"
          />
        </template>

        <template v-else-if="activeTab === 'about'">
          <AboutTab
            :intro="activeAdvancedIntro"
            :version="version"
            :check-result-label="checkResultLabel"
            :update-checking="updateChecking"
            :is-downloading="updateState.kind === 'downloading'"
            @check-updates="onCheckUpdates"
          />
        </template>

        <template v-else-if="activeTab === 'debug'">
          <DebugTab
            :developer-mode="developerMode"
            :launcher-state-ttl="launcherStateTtl"
            :backend="backend"
            :crash-files="crashFiles"
            :crashes-error="crashesError"
            :bundle-saved-path="bundleSavedPath"
            :bundling="bundling"
            :bundle-error="bundleError"
            @toggle-developer-mode="onToggleDeveloperMode"
            @launcher-state-ttl-change="onLauncherStateTtlChange"
            @open-crashes-folder="onOpenCrashesFolder"
            @clear-crashes="onClearCrashes"
            @bundle-save="onBundleSave"
            @open-logs-folder="onOpenLogsFolder"
          />
        </template>

        <template v-else-if="activeTab === 'security'">
          <SecurityTab />
        </template>

        <template v-else-if="activeTab === 'ai'">
          <AISettingsTab
            :intro="activeAdvancedIntro"
            :view="aiSettingsView"
            @update:view="onAiSettingsViewChange"
          />
        </template>

        <template v-else-if="activeTab === 'secrets'">
          <SecretsTab />
        </template>

        <template v-else-if="activeTab === 'sync'">
          <SyncTab />
        </template>

        <template v-else-if="activeTab === 'dictation'">
          <DictationTab
            :intro="activeAdvancedIntro"
            :commands="activeAppCommands"
            :usage-tracker="usageTracker"
            :is-command-visible="isCommandVisible"
            @toggle-usage-tracker="onToggleUsageTracker"
            @toggle-command-visibility="onToggleCommandVisibility"
          />
        </template>

        <template
          v-else-if="
            activeTab === 'notes' ||
            activeTab === 'tasks' ||
            activeTab === 'time-tracker' ||
            activeTab === 'games'
          "
        >
          <AppCommandsTab
            :intro="activeAdvancedIntro"
            :active-tab="activeTab"
            :commands="activeAppCommands"
            :usage-tracker="usageTracker"
            :is-command-visible="isCommandVisible"
            @toggle-usage-tracker="onToggleUsageTracker"
            @toggle-command-visibility="onToggleCommandVisibility"
          />
        </template>

        <template v-else-if="activeTab === 'file-search'">
          <FileSearchTab :key="fileSearchTabRevision" :intro="activeAdvancedIntro" />
        </template>

        <template v-else-if="activeTab === 'clipboard'">
          <ClipboardSettingsTab :intro="activeAdvancedIntro" />
        </template>

        <!-- Extensions tab — плоский список установленных. Обновления подтягиваются из catalog.json. -->
        <template v-else-if="activeTab === 'extensions'">
          <ExtensionsTab :intro="activeAdvancedIntro" />
        </template>

        <!-- Focus tab — управление блок-листами доменов и активной блокировкой -->
        <template v-else-if="activeTab === 'focus'">
          <FocusTab :intro="activeAdvancedIntro" />
        </template>

        <!-- Export tab — список зарегистрированных converters + history -->
        <template v-else-if="activeTab === 'export'">
          <ExportTab />
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Все «утилитарные» классы (`.settings-shell`, `.row`, `.btn`, `.label`,
   `.hint`, `.advanced-page`, `.chrome-control`, `.stats-card*`, и т.д.)
   переехали в `./settings/settings-shared.css`. Они namespaced под
   `.settings-shell`, поэтому работают и в дочерних tab-компонентах без
   `:deep()` хаков. Здесь ниже остаются только tab-specific классы. */

/* Все scoped tab-specific CSS переехали в соответствующие per-tab компоненты:
 * - LegacyToggle / UpdateBanner (shell-local components)
 * - AppCommandsTab / ExtensionsTab / FileSearchTab / FocusTab / ExportTab
 *   (per-tab .vue с собственным scoped-style)
 *
 * Утилитарные классы (.row, .btn, .label, .hint, .advanced-page и т.д.)
 * живут в `./settings/settings-shared.css` (non-scoped, namespaced под
 * .settings-shell). */

/* macOS Raycast-style layout: навигация в top-tab баре, контент во всю ширину
   под ним. Sidebar скрыт (v-if), поэтому settings-content занимает всё. */
.settings--mac .settings-content {
  padding-top: 4px;
}
</style>
