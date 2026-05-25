<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, type Component } from "vue";
import {
  BookOpen,
  Bug,
  Check,
  ChevronLeft,
  ChevronRight,
  Gamepad2,
  Info,
  ListTodo,
  KeyRound,
  Mic,
  Puzzle,
  Search,
  Settings,
  Shield,
  ShieldCheck,
  Timer,
} from "@lucide/vue";
import LegacyToggle from "./settings/components/LegacyToggle.vue";
import UpdateBanner from "./settings/components/UpdateBanner.vue";
import AboutTab from "./settings/tabs/AboutTab.vue";
import DebugTab from "./settings/tabs/DebugTab.vue";
import DictationTab from "./settings/tabs/DictationTab.vue";
import ExportTab from "./settings/tabs/ExportTab.vue";
import ExtensionsTab from "./settings/tabs/ExtensionsTab.vue";
import FileSearchTab from "./settings/tabs/FileSearchTab.vue";
import FocusTab from "./settings/tabs/FocusTab.vue";
import GeneralTab from "./settings/tabs/GeneralTab.vue";
import SecretsTab from "./settings/tabs/SecretsTab.vue";
import SecurityTab from "./settings/tabs/SecurityTab.vue";
import {
  createDictationConfig,
  DictationConfigKey,
} from "./settings/composables/useDictationConfig";
import { useKeplerUpdate } from "./settings/composables/useKeplerUpdate";
import "./settings/settings-shared.css";
import holoSvg from "../assets/holo.svg";
import holoPomoSvg from "../assets/holo-pomo.svg";
import holoSecoSvg from "../assets/holo-seco.svg";
import horoLogoPng from "../assets/horo-logo.png";
import delphiSvg from "../assets/delphi.svg";
import delphiAddSvg from "../assets/delphi-add.svg";
import arraSvg from "../assets/arra.svg";
import edenSvg from "../assets/eden.svg";
import edenAddSvg from "../assets/eden-add.svg";
import edenDiarySvg from "../assets/eden-diary.svg";
import kosmosIconPng from "../../build/icon.png";
import {
  SettingsAdvancedIntro,
  SettingsSearchInput,
  SettingsSidebar,
  SettingsSidebarButton,
  ToastHost,
  provideToastHost,
} from "@kosmos/visuals";
import type { BackendStatus } from "@shared/ipc-types";

type Tab =
  | "general"
  | "about"
  | "debug"
  | "security"
  | "secrets"
  | "notes"
  | "tasks"
  | "time-tracker"
  | "games"
  | "extensions"
  | "focus"
  | "dictation"
  | "file-search"
  | "export";

interface SettingsNavigationItem {
  tab: Tab;
  label: string;
  group: "main" | "advanced";
  layout: "basic" | "advanced";
  icon: Component;
  iconGradient?: {
    from: string;
    to: string;
  };
  sidebarImage?: string;
  introImage?: string;
  description?: string;
  keywords: string[];
}

type AppSettingsTab = "notes" | "tasks" | "time-tracker" | "games";

interface AppCommandSetting {
  id: string;
  title: string;
  icon: string;
  iconFrom: string;
  iconTo: string;
}

const HIDDEN_COMMANDS_KEY = "kepler.launcher.hiddenCommandIds";
const EDEN_COMMAND_GRADIENT = { iconFrom: "#ff5c00", iconTo: "#b33800" };
const DELPHI_COMMAND_GRADIENT = {
  iconFrom: "oklch(0.78 0.14 230)",
  iconTo: "oklch(0.5 0.18 245)",
};
const HOROLOGION_COMMAND_GRADIENT = {
  iconFrom: "oklch(0.66 0.245 305)",
  iconTo: "oklch(0.42 0.20 305)",
};
const ARRANCADOR_COMMAND_GRADIENT = {
  iconFrom: "oklch(0.7 0.2 25)",
  iconTo: "oklch(0.45 0.18 20)",
};

const appCommandSettings: Record<AppSettingsTab, AppCommandSetting[]> = {
  notes: [
    {
      id: "eden:open",
      title: "Открыть Eden",
      icon: edenSvg,
      ...EDEN_COMMAND_GRADIENT,
    },
    {
      id: "eden:note:create",
      title: "Создать заметку",
      icon: edenAddSvg,
      ...EDEN_COMMAND_GRADIENT,
    },
    {
      id: "eden:note:open-today",
      title: "Открыть сегодняшнюю заметку",
      icon: edenDiarySvg,
      ...EDEN_COMMAND_GRADIENT,
    },
  ],
  tasks: [
    {
      id: "delphi:open",
      title: "Открыть Delphi",
      icon: delphiSvg,
      ...DELPHI_COMMAND_GRADIENT,
    },
    {
      id: "delphi:inbox",
      title: "Открыть входящие",
      icon: delphiAddSvg,
      ...DELPHI_COMMAND_GRADIENT,
    },
    {
      id: "delphi:task:create",
      title: "Создать задачу",
      icon: delphiAddSvg,
      ...DELPHI_COMMAND_GRADIENT,
    },
    {
      id: "delphi:task:today",
      title: "Открыть сегодняшние задачи",
      icon: delphiSvg,
      ...DELPHI_COMMAND_GRADIENT,
    },
  ],
  "time-tracker": [
    {
      id: "horologion:open",
      title: "Открыть Horologion",
      icon: holoSvg,
      ...HOROLOGION_COMMAND_GRADIENT,
    },
    {
      id: "horologion:pomodoro:25",
      title: "Помодоро 25 минут",
      icon: holoPomoSvg,
      ...HOROLOGION_COMMAND_GRADIENT,
    },
    {
      id: "horologion:pomodoro:50",
      title: "Помодоро 50 минут",
      icon: holoPomoSvg,
      ...HOROLOGION_COMMAND_GRADIENT,
    },
    {
      id: "horologion:stopwatch:start",
      title: "Запустить секундомер",
      icon: holoSecoSvg,
      ...HOROLOGION_COMMAND_GRADIENT,
    },
  ],
  games: [
    {
      id: "arrancador:open",
      title: "Открыть Arrancador",
      icon: arraSvg,
      ...ARRANCADOR_COMMAND_GRADIENT,
    },
  ],
};

const settingsNavigationItems: SettingsNavigationItem[] = [
  {
    tab: "general",
    label: "Общие",
    group: "main",
    layout: "basic",
    icon: Settings,
    keywords: [
      "общие",
      "настройки",
      "глобальный хоткей",
      "launcher",
      "автозапуск",
      "windows",
      "трей",
      "tray",
    ],
  },
  {
    tab: "security",
    label: "Безопасность",
    group: "main",
    layout: "basic",
    icon: ShieldCheck,
    description: "Сеть для AI-провайдеров и доступ к данным.",
    keywords: ["безопасность", "сеть", "dns", "doh", "ai", "groq", "прокси", "блокировки", "рф"],
  },
  {
    tab: "secrets",
    label: "Секреты",
    group: "main",
    layout: "basic",
    icon: KeyRound,
    description: "API-ключи для AI-провайдеров. Хранятся в Windows Credential Manager.",
    keywords: ["секреты", "api", "ключ", "key", "credential", "groq", "token", "пароль"],
  },
  {
    tab: "debug",
    label: "Дебаг",
    group: "main",
    layout: "basic",
    icon: Bug,
    keywords: [
      "дебаг",
      "debug",
      "developer mode",
      "режим разработчика",
      "backend",
      "lock файл",
      "отчеты об ошибках",
      "bug report",
      "логи",
      "crash",
      "позиция в лаунчере",
    ],
  },
  {
    tab: "about",
    label: "О приложении",
    group: "main",
    layout: "basic",
    icon: Info,
    introImage: kosmosIconPng,
    description: "Kepler shell и обновления приложения.",
    keywords: ["about", "о приложении", "версия", "kepler", "kosmos", "обновления", "update"],
  },
  {
    tab: "file-search",
    label: "Поиск файлов",
    group: "advanced",
    layout: "advanced",
    icon: Search,
    iconGradient: { from: "#94A3B8", to: "#334155" },
    description: "Индексация локальных файлов и исключения шумных папок.",
    keywords: [
      "поиск файлов",
      "file search",
      "индексация файлов",
      "шумные папки",
      "node_modules",
      ".git",
      "переиндексация",
      "локальные диски",
    ],
  },
  {
    tab: "extensions",
    label: "Расширения",
    group: "advanced",
    layout: "advanced",
    icon: Puzzle,
    iconGradient: { from: "#A78BFA", to: "#5B21B6" },
    description: "Установленные расширения, каталог и управление приложениями.",
    keywords: [
      "расширения",
      "каталог",
      "marketplace",
      "установленные",
      "список приложений",
      "eden",
      "delphi",
      "arrancador",
      "horologion",
    ],
  },
  {
    tab: "dictation",
    label: "Диктация",
    group: "advanced",
    layout: "advanced",
    icon: Mic,
    iconGradient: { from: "#F472B6", to: "#7C2D12" },
    description: "Голосовой ввод через Groq (whisper-large-v3-turbo).",
    keywords: [
      "диктация",
      "stt",
      "голос",
      "whisper",
      "groq",
      "распознавание речи",
      "транскрипция",
      "voice",
    ],
  },
  {
    tab: "focus",
    label: "Фокус",
    group: "advanced",
    layout: "advanced",
    icon: Shield,
    iconGradient: { from: "#60A5FA", to: "#1D4ED8" },
    description: "Блокировка отвлечений и системный демон фокус-режима.",
    keywords: [
      "фокус",
      "блокировка",
      "системный демон",
      "focus service",
      "активная блокировка",
      "blocklist",
      "домены",
      "сайты",
      "hosts",
      "pomodoro",
    ],
  },
  {
    tab: "tasks",
    label: "Задачи",
    group: "advanced",
    layout: "advanced",
    icon: ListTodo,
    iconGradient: { from: "#2DD4BF", to: "#0F766E" },
    description: "Delphi: задачи, списки и рабочие действия.",
    keywords: ["задачи", "delphi", "todo", "task", "списки", "дела"],
  },
  {
    tab: "notes",
    label: "Заметки",
    group: "advanced",
    layout: "advanced",
    icon: BookOpen,
    iconGradient: { from: "#7C5CFF", to: "#3A237D" },
    description: "Eden: заметки, дневник и быстрый доступ к текстам.",
    keywords: ["заметки", "eden", "дневник", "note", "journal", "тексты"],
  },
  {
    tab: "time-tracker",
    label: "Времяметр",
    group: "advanced",
    layout: "advanced",
    icon: Timer,
    iconGradient: { from: "#F59E0B", to: "#92400E" },
    sidebarImage: horoLogoPng,
    introImage: horoLogoPng,
    description: "Horologion: учёт времени, сессии и pomodoro.",
    keywords: [
      "времяметр",
      "трекер времени",
      "horologion",
      "time tracker",
      "pomodoro",
      "таймер",
      "трекать активные приложения",
      "usage tracker",
    ],
  },
  {
    tab: "games",
    label: "Игры",
    group: "advanced",
    layout: "advanced",
    icon: Gamepad2,
    iconGradient: { from: "#F472B6", to: "#9D174D" },
    description: "Arrancador: библиотека игр и связанные настройки.",
    keywords: ["игры", "arrancador", "games", "game library", "библиотека игр"],
  },
];

// Focus-tab types/constants — `./settings/composables/useFocusTab.ts`.

const tab = ref<Tab>("general");
const searchQuery = ref<string>("");

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

const showAdvancedToggle = computed(() => activeLayout.value === "advanced");

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
  return value === "notes" || value === "tasks" || value === "time-tracker" || value === "games";
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

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    onClose();
  }
}

// All tab data loading инкапсулировано в их composables (useExportTab,
// useFocusTab, useExtensionsTab, useFileSearchTab, useDictationConfig).
// SettingsView больше не загружает tab data сам.

function selectTab(t: Tab) {
  tab.value = t;
}


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
  <div class="settings" tabindex="0" @keydown="onKey">
    <ToastHost />
    <!-- Raycast-style update banner. Шириной во всё окно, height ~32px. -->
    <UpdateBanner :banner="updateBanner" :state="updateState" @install="onInstallUpdate" />

    <div class="settings-shell">
      <SettingsSidebar title="Настройки">
        <div class="settings-sidebar-group">
          <SettingsSearchInput v-model="searchQuery" placeholder="Поиск" />
        </div>

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
      </SettingsSidebar>

      <div class="settings-content">
        <header class="content-header">
          <div class="content-header__nav">
            <button type="button" class="chrome-control" disabled title="Назад" aria-label="Назад">
              <ChevronLeft :size="14" :stroke-width="2" />
            </button>
            <button
              type="button"
              class="chrome-control"
              disabled
              title="Вперёд"
              aria-label="Вперёд"
            >
              <ChevronRight :size="14" :stroke-width="2" />
            </button>
          </div>

          <div class="content-header__right">
            <button
              v-if="showAdvancedToggle"
              type="button"
              class="advanced-feature-toggle"
              disabled
              role="switch"
              aria-checked="false"
              aria-label="Включить функцию"
              title="Скоро можно будет включать и выключать эту функцию"
            >
              <span class="advanced-feature-toggle__thumb" />
            </button>
          </div>
        </header>

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

        <template v-else-if="activeTab === 'secrets'">
          <SecretsTab />
        </template>

        <template v-else-if="activeTab === 'dictation'">
          <DictationTab :intro="activeAdvancedIntro" />
        </template>

        <template
          v-else-if="
            activeTab === 'notes' ||
            activeTab === 'tasks' ||
            activeTab === 'time-tracker' ||
            activeTab === 'games'
          "
        >
          <section class="advanced-page kosmos-scroll">
            <SettingsAdvancedIntro
              v-if="activeAdvancedIntro"
              :icon="activeAdvancedIntro.icon"
              :title="activeAdvancedIntro.label"
              :description="activeAdvancedIntro.description"
              :image-src="activeAdvancedIntro.introImage"
              :icon-from="activeAdvancedIntro.iconGradient?.from"
              :icon-to="activeAdvancedIntro.iconGradient?.to"
            />

            <div class="advanced-page__body">
              <div
                v-if="activeTab === 'time-tracker'"
                class="rows command-settings-list time-settings-list"
              >
                <div class="row">
                  <div class="row-label">
                    <div class="label">Трекать активные приложения</div>
                    <div class="hint">
                      Записывает в ARK какое окно сейчас активно. Изменение применится после
                      перезапуска Kepler.
                    </div>
                  </div>
                  <LegacyToggle :checked="usageTracker" @change="onToggleUsageTracker" />
                </div>
              </div>

              <div>
                <h2 class="advanced-section-title">Команды</h2>
                <div class="rows command-settings-list">
                  <div v-for="command in activeAppCommands" :key="command.id" class="row">
                    <div class="command-row-label">
                      <span
                        class="command-row-icon"
                        :style="{
                          '--command-row-icon-from': command.iconFrom,
                          '--command-row-icon-to': command.iconTo,
                        }"
                        aria-hidden="true"
                      >
                        <img :src="command.icon" alt="" />
                      </span>
                      <div class="label">{{ command.title }}</div>
                    </div>
                    <label class="command-checkbox" :aria-label="`Показывать ${command.title}`">
                      <input
                        type="checkbox"
                        :checked="isCommandVisible(command.id)"
                        @change="onToggleCommandVisibility(command.id, $event)"
                      />
                      <span class="command-checkbox__box" aria-hidden="true">
                        <Check :size="13" :stroke-width="3" />
                      </span>
                    </label>
                  </div>
                </div>
              </div>
            </div>
          </section>
        </template>

        <template v-else-if="activeTab === 'file-search'">
          <FileSearchTab :intro="activeAdvancedIntro" />
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

.command-settings-list {
  width: 100%;
}

.time-settings-list {
  margin-top: 16px;
}

.command-row-label {
  display: inline-flex;
  min-width: 0;
  align-items: center;
  gap: 10px;
}

.command-row-icon {
  display: inline-flex;
  width: 22px;
  height: 22px;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border-radius: 4px;
  background-image: linear-gradient(
    to bottom left,
    var(--command-row-icon-from),
    var(--command-row-icon-to)
  );
  box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 6%, transparent);
  overflow: hidden;
}

.command-row-icon img {
  display: block;
  width: 16px;
  height: 16px;
  object-fit: contain;
}

.command-checkbox {
  position: relative;
  display: inline-flex;
  width: 20px;
  height: 20px;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  cursor: pointer;
}

.command-checkbox input {
  position: absolute;
  width: 0;
  height: 0;
  opacity: 0;
  pointer-events: none;
}

.command-checkbox__box {
  display: inline-flex;
  width: 16px;
  height: 16px;
  align-items: center;
  justify-content: center;
  border: 1px solid color-mix(in srgb, var(--foreground) 24%, transparent);
  border-radius: 4px;
  background: transparent;
  color: var(--main-background-color);
  transition:
    background 120ms ease,
    border-color 120ms ease,
    color 120ms ease;
}

.command-checkbox__box svg {
  opacity: 0;
  transform: scale(0.82);
  transition:
    opacity 120ms ease,
    transform 120ms ease;
}

.command-checkbox input:checked + .command-checkbox__box {
  border-color: var(--foreground);
  background: var(--foreground);
  color: #0d0d0d;
}

.command-checkbox input:checked + .command-checkbox__box svg {
  opacity: 1;
  transform: scale(1);
}

.command-checkbox input:focus-visible + .command-checkbox__box {
  outline: 2px solid color-mix(in srgb, var(--foreground) 35%, transparent);
  outline-offset: 2px;
}

/* `.toggle`/`.track`/`.thumb` стили переехали в
   `./settings/components/LegacyToggle.vue`. */

/* Extensions tab CSS переехал в `./settings/tabs/ExtensionsTab.vue`. */

/* FileSearch tab CSS переехал в `./settings/tabs/FileSearchTab.vue`. */
</style>
