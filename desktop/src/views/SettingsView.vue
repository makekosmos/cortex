<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  settingsNavigationItems,
  type SettingsNavigationItem,
  type Tab,
} from "./settings/navigation";
import UpdateBanner from "./settings/components/UpdateBanner.vue";
import AboutTab from "./settings/tabs/AboutTab.vue";
import DebugTab from "./settings/tabs/DebugTab.vue";
import ExportTab from "./settings/tabs/ExportTab.vue";
import FileIndexTab from "./settings/tabs/FileIndexTab.vue";
import GeneralTab from "./settings/tabs/GeneralTab.vue";
import { useKeplerUpdate } from "./settings/composables/useKeplerUpdate";
import "@kosmos/visuals/components/settings-shell.css";
import "./settings/settings-shared.css";
import {
  SettingsContentHeader,
  SettingsSearchInput,
  SettingsSidebar,
  SettingsSidebarButton,
  usePlatform,
} from "@kosmos/visuals";
import SettingsTopTabs from "./settings/components/SettingsTopTabs.vue";

// macOS: настройки в стиле Raycast — горизонтальный таб-бар сверху вместо
// вертикального сайдбара. Те же navigation items, другой layout.
const { isMac } = usePlatform();

const tab = ref<Tab>("general");
const searchQuery = ref<string>("");
type SettingsLocation = { tab: Tab };
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

const canGoBack = computed(() => backStack.value.length > 0);
const canGoForward = computed(() => forwardStack.value.length > 0);

const activeAdvancedIntro = computed(() => {
  const item = activeNavigationItem.value;
  if (!item || (item.layout !== "advanced" && !item.introImage)) return null;
  return item;
});

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
const trayIcon = ref<boolean>(true);
const launcherStateTtl = ref<number>(5);
const loading = ref<boolean>(true);

async function loadGeneral() {
  loading.value = true;
  try {
    const [h, v, ttl] = await Promise.all([
      window.kepler.settings.hotkey(),
      window.kepler.settings.version(),
      window.kepler.settings.launcherStateTtl.get(),
    ]);
    hotkey.value = h;
    version.value = v;
    launcherStateTtl.value = ttl;
  } catch (e) {
    console.warn("settings load failed", e);
  } finally {
    loading.value = false;
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
  // SAFETY: the surrounding domain validation preserves the asserted contract.
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

async function onLauncherStateTtlChange(e: Event) {
  // SAFETY: the surrounding domain validation preserves the asserted contract.
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

function onClose() {
  void window.kepler.settings.close();
}

function currentLocation(): SettingsLocation {
  return { tab: tab.value };
}

function sameLocation(left: SettingsLocation, right: SettingsLocation): boolean {
  return left.tab === right.tab;
}

function applyLocation(location: SettingsLocation) {
  tab.value = location.tab;
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    onClose();
  }
}

// All remaining tab data loading инкапсулировано в their composables.
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
  navigateToLocation({ tab: t }, recordHistory);
}

function selectTab(t: Tab) {
  navigateToTab(t);
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

watch(activeTab, (next) => {
  if (!searchQuery.value || !next) return;
  if (tab.value !== next) {
    tab.value = next;
  }
});

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

onMounted(() => {
  void loadGeneral();
  void loadTrayIcon();
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
  <div class="settings" :class="{ 'settings--mac': isMac }" tabindex="0" @keydown="onKey">
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
      <SettingsSidebar v-if="!isMac" title="Настройки" background="var(--bg-app)">
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
        </SettingsContentHeader>

        <div v-if="searchQuery && !activeTab" class="empty">Ничего не найдено</div>

        <!-- General tab -->
        <template v-else-if="activeTab === 'general'">
          <GeneralTab
            :loading="loading"
            :hotkey="hotkey"
            :hotkey-error="hotkeyError"
            :tray-icon="trayIcon"
            @launcher-hotkey-change="onLauncherHotkeyChange"
            @reset-hotkey="resetHotkey"
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
            :launcher-state-ttl="launcherStateTtl"
            @launcher-state-ttl-change="onLauncherStateTtlChange"
          />
        </template>

        <template v-else-if="activeTab === 'file-index'">
          <FileIndexTab />
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
* - AppCommandsTab / ExportTab
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
