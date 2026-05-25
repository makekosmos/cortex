<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import {
  BookOpen,
  Bug,
  Check,
  ChevronLeft,
  ChevronRight,
  Folder,
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
import UpdateBanner from "./settings/components/UpdateBanner.vue";
import { useKeplerUpdate } from "./settings/composables/useKeplerUpdate";
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
  BlocklistCard,
  Button,
  HotkeyCapture,
  RadioGroup,
  SettingsAdvancedIntro,
  SettingsDropdownRow,
  SettingsList,
  SettingsRow,
  SettingsSearchInput,
  SettingsSidebar,
  SettingsSidebarButton,
  Textarea,
  TextInput,
  ToastHost,
  provideToastHost,
} from "@kosmos/visuals";
import type {
  BackendStatus,
  ExportConverterInfo,
  ExportResult,
  FileIndexSettings,
  InstalledExtensionInfo,
  MarketplaceCatalog,
  MarketplaceExtension,
} from "@shared/ipc-types";

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

interface FocusBlocklist {
  id: string;
  name: string;
  domains: string[];
  createdAt: string;
  preset?: boolean;
  icon?: string;
  kind?: "domains" | "raw";
}

const ICON_CHOICES = [
  "🛡️",
  "🚫",
  "🎮",
  "🧠",
  "📰",
  "📺",
  "🎬",
  "💬",
  "🐦",
  "📷",
  "🛒",
  "⚽",
  "🎰",
  "🍔",
  "💸",
  "🎵",
  "📚",
  "⚙️",
  "🔒",
  "🎯",
  "⏰",
  "🌐",
  "✨",
  "🔥",
  "⚡",
];

interface FocusActiveState {
  active: boolean;
  blocklist_id?: string | null;
  started_at?: string | null;
}

const DOMAIN_PATTERN = /^[a-z0-9][a-z0-9.-]*\.[a-z]{2,}$/i;

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
const fileSearchSettings = ref<FileIndexSettings | null>(null);
const fileSearchBusy = ref<boolean>(false);
const fileSearchError = ref<string>("");
const fileSearchNewIgnore = ref<string>("");
const { api: toast } = provideToastHost();
let fileSearchPollTimer: ReturnType<typeof window.setTimeout> | null = null;
let fileSearchToastId: number | null = null;
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
    await loadFileSearchSettings();
  } catch (e) {
    console.warn("settings load failed", e);
  } finally {
    loading.value = false;
  }
}

async function loadFileSearchSettings() {
  fileSearchError.value = "";
  try {
    fileSearchSettings.value = await window.kepler.fileSearch.settingsGet();
  } catch (err) {
    console.warn("file_index.settings_get failed", err);
    fileSearchError.value = "Настройки поиска файлов пока недоступны";
  }
}

function clearFileSearchPoll() {
  if (fileSearchPollTimer) {
    window.clearTimeout(fileSearchPollTimer);
    fileSearchPollTimer = null;
  }
  // Regression M6 (2026-05-24): old code left the previous progress toast
  // around when a new operation kicked in — stack grew, all stuck with
  // duration:0.
  if (fileSearchToastId !== null) {
    toast.dismiss(fileSearchToastId);
    fileSearchToastId = null;
  }
}

function watchFileSearchProgress(message = "Индексация файлов запущена") {
  // Regression 2026-05-24-evening: order matters. clearFileSearchPoll dismisses
  // the toast it sees in fileSearchToastId — so it MUST run before we assign
  // the new id, otherwise we dismiss the toast we just created.
  clearFileSearchPoll();
  fileSearchToastId = toast.show({
    title: "Поиск файлов",
    message,
    description: formatFileSearchProgress(fileSearchSettings.value),
    tone: "info",
    duration: 0,
    loading: true,
    closable: true,
  });
  const startedAt = Date.now();
  const poll = async () => {
    try {
      const next = await window.kepler.fileSearch.settingsGet();
      fileSearchSettings.value = next;
      if (fileSearchToastId !== null) {
        toast.update(fileSearchToastId, {
          title: "Индексируем файлы",
          message: next.scan_progress.message || "Индексация файлов",
          description: formatFileSearchProgress(next),
          tone: "info",
          loading: true,
          duration: 0,
          closable: true,
        });
      }
      if (!next.scan_in_progress) {
        if (fileSearchToastId !== null) {
          toast.update(fileSearchToastId, {
            title: "Поиск файлов",
            message: "Индексация завершена",
            description: formatFileSearchProgress(next),
            tone: "success",
            loading: false,
            duration: 2600,
            closable: true,
          });
          fileSearchToastId = null;
        }
        fileSearchPollTimer = null;
        return;
      }
    } catch (err) {
      console.warn("file_index progress poll failed", err);
      if (Date.now() - startedAt > 90_000) {
        if (fileSearchToastId !== null) {
          toast.update(fileSearchToastId, {
            title: "Поиск файлов",
            message: "Индексация продолжается в фоне",
            description: "Статус обновится при следующем открытии настроек.",
            loading: false,
            closable: true,
            duration: 4200,
          });
          fileSearchToastId = null;
        } else {
          toast.show({
            title: "Поиск файлов",
            message: "Индексация продолжается в фоне, статус обновится позже",
            tone: "info",
            duration: 3200,
          });
        }
        fileSearchPollTimer = null;
        return;
      }
    }
    fileSearchPollTimer = window.setTimeout(poll, 2000);
  };
  fileSearchPollTimer = window.setTimeout(poll, 1200);
}

function formatFileSearchProgress(settings: FileIndexSettings | null): string {
  const progress = settings?.scan_progress;
  if (!progress) return "Ожидаем статус индексатора.";
  const parts: string[] = [];
  if (progress.root) {
    parts.push(shortenPath(progress.root));
  }
  if (progress.roots_total > 0) {
    parts.push(
      `папка ${Math.min(progress.roots_done + 1, progress.roots_total)}/${progress.roots_total}`,
    );
  }
  if (progress.files_seen > 0 || progress.files_indexed > 0) {
    parts.push(`${formatCount(progress.files_indexed || progress.files_seen)} файлов`);
  }
  if (progress.phase === "ntfs") {
    parts.push("NTFS scan");
  }
  return parts.length > 0 ? parts.join(" · ") : "Индексатор готовится.";
}

function formatCount(value: number): string {
  return new Intl.NumberFormat("ru-RU").format(value);
}

function shortenPath(path: string): string {
  if (path.length <= 42) return path;
  return `${path.slice(0, 18)}…${path.slice(-20)}`;
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

async function onToggleFileSearchNoise(e: Event) {
  const target = e.target as HTMLInputElement;
  await updateFileSearchSettings({ exclude_noisy_folders: target.checked });
}

async function onToggleFileSearchGitignore(e: Event) {
  const target = e.target as HTMLInputElement;
  await updateFileSearchSettings({ respect_gitignore: target.checked });
}

async function onToggleFileSearchHidden(e: Event) {
  const target = e.target as HTMLInputElement;
  await updateFileSearchSettings({ include_hidden: target.checked });
}

async function onToggleFileSearchNtfs(e: Event) {
  const target = e.target as HTMLInputElement;
  await updateFileSearchSettings({ ntfs_accelerated: target.checked });
}

async function updateFileSearchSettings(patch: {
  exclude_noisy_folders?: boolean;
  respect_gitignore?: boolean;
  include_hidden?: boolean;
  ntfs_accelerated?: boolean;
}) {
  // Regression M2 (2026-05-24): optimistic update so the toggle visually
  // stays where the user put it instead of flipping back to the server value
  // for a moment. On error we reload from server (rollback).
  const snapshot = fileSearchSettings.value ? { ...fileSearchSettings.value } : null;
  if (fileSearchSettings.value) {
    fileSearchSettings.value = { ...fileSearchSettings.value, ...patch };
  }
  fileSearchBusy.value = true;
  fileSearchError.value = "";
  try {
    await window.kepler.fileSearch.settingsSet(patch);
    await loadFileSearchSettings();
    if (fileSearchSettings.value?.scan_in_progress) {
      watchFileSearchProgress("Настройка сохранена, индекс обновляется");
    }
  } catch (err) {
    console.warn("file_index.settings_set failed", err);
    if (snapshot) fileSearchSettings.value = snapshot;
    fileSearchError.value = describeFileSearchError(err, "Не удалось применить настройку");
    await loadFileSearchSettings();
  } finally {
    fileSearchBusy.value = false;
  }
}

async function onAddFileSearchScope() {
  fileSearchError.value = "";
  const picked = await window.kepler.fileSearch.pickScope();
  if (!picked) return;
  fileSearchBusy.value = true;
  try {
    await window.kepler.fileSearch.scopeAdd(picked);
    await loadFileSearchSettings();
    watchFileSearchProgress("Папка добавлена, индексация запущена");
  } catch (err) {
    console.warn("file_index.scope_add failed", err);
    fileSearchError.value = "Не удалось добавить папку поиска";
  } finally {
    fileSearchBusy.value = false;
  }
}

function describeFileSearchError(err: unknown, fallback: string): string {
  // Regression H7 (2026-05-24): backend errors used to be swallowed into a
  // generic toast — user couldn't tell "duplicate pattern" from "invalid glob".
  const raw = (err as { message?: string } | null)?.message ?? String(err ?? "");
  const trimmed = raw.trim();
  if (!trimmed) return fallback;
  const known = [
    "pattern уже есть",
    "invalid ignore pattern",
    "must not be empty",
    "must be an existing directory",
  ];
  for (const marker of known) {
    if (trimmed.toLowerCase().includes(marker.toLowerCase())) {
      return trimmed.replace(/^[a-z_]+\.[a-z_]+:\s*/i, "");
    }
  }
  return `${fallback}: ${trimmed}`;
}

async function onRemoveFileSearchScope(path: string) {
  // Regression H5 (2026-05-24): scope removal triggers index cleanup of
  // potentially thousands of files. Without confirm an accidental click
  // wipes hours of scan work.
  const confirmed = window.confirm(
    `Удалить папку поиска «${path}»? Все её проиндексированные файлы будут удалены.`,
  );
  if (!confirmed) return;
  fileSearchBusy.value = true;
  fileSearchError.value = "";
  try {
    await window.kepler.fileSearch.scopeRemove(path);
    await loadFileSearchSettings();
    watchFileSearchProgress("Папка удалена, индекс обновляется");
  } catch (err) {
    console.warn("file_index.scope_remove failed", err);
    fileSearchError.value = describeFileSearchError(err, "Не удалось удалить папку поиска");
  } finally {
    fileSearchBusy.value = false;
  }
}

async function onAddFileSearchIgnore() {
  const pattern = fileSearchNewIgnore.value.trim();
  if (!pattern) return;
  // Regression H7 (2026-05-24): frontend-side dedup so user gets immediate
  // feedback without a backend round-trip.
  const existing = fileSearchSettings.value?.ignore_patterns ?? [];
  if (existing.some((p) => p.toLowerCase() === pattern.toLowerCase())) {
    fileSearchError.value = `Шаблон уже добавлен: ${pattern}`;
    return;
  }
  fileSearchBusy.value = true;
  fileSearchError.value = "";
  try {
    await window.kepler.fileSearch.ignoreAdd(pattern);
    fileSearchNewIgnore.value = "";
    await loadFileSearchSettings();
    watchFileSearchProgress("Шаблон добавлен, индекс обновляется");
  } catch (err) {
    console.warn("file_index.ignore_add failed", err);
    fileSearchError.value = describeFileSearchError(err, "Не удалось добавить шаблон");
  } finally {
    fileSearchBusy.value = false;
  }
}

async function onRemoveFileSearchIgnore(pattern: string) {
  fileSearchBusy.value = true;
  fileSearchError.value = "";
  try {
    await window.kepler.fileSearch.ignoreRemove(pattern);
    await loadFileSearchSettings();
    watchFileSearchProgress("Шаблон удалён, индекс обновляется");
  } catch (err) {
    console.warn("file_index.ignore_remove failed", err);
    fileSearchError.value = describeFileSearchError(err, "Не удалось удалить шаблон");
  } finally {
    fileSearchBusy.value = false;
  }
}

async function onRescanFileSearch() {
  // Regression M7 (2026-05-24): block if a scan is already running — the old
  // code spawned redundant rescan jobs that queued on scan_lock.
  if (fileSearchSettings.value?.scan_in_progress) {
    fileSearchError.value = "Индексация уже идёт";
    return;
  }
  fileSearchBusy.value = true;
  fileSearchError.value = "";
  try {
    await window.kepler.fileSearch.rescan();
    await loadFileSearchSettings();
    watchFileSearchProgress("Переиндексация запущена");
  } catch (err) {
    console.warn("file_index.rescan failed", err);
    fileSearchError.value = describeFileSearchError(err, "Не удалось переиндексировать файлы");
  } finally {
    fileSearchBusy.value = false;
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
  const format = exportSelectedFormat.value[c.converter_id] ?? c.default_format;
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
    const parts: string[] = [`Готово: ${r.files_written.length} файлов, ${sizeKb} KB`];
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

// --- Focus ------------------------------------------------------------------

const focusBlocklists = ref<FocusBlocklist[]>([]);
const focusLoading = ref<boolean>(false);
const focusError = ref<string>("");
const focusBackendMissing = ref<boolean>(false);
const focusActive = ref<FocusActiveState>({ active: false });
const focusBusy = ref<string>("");

// Service daemon status — для secure zero-UAC focus mode.
const focusServiceStatus = ref<{ installed: boolean; running: boolean }>({
  installed: false,
  running: false,
});
const focusServiceBusy = ref<string>("");
const focusServiceError = ref<string>("");
let unsubscribeFocusServiceStatus: (() => void) | null = null;

async function refreshFocusServiceStatus() {
  try {
    focusServiceStatus.value = await window.kepler.focusService.status();
  } catch (e) {
    console.warn("focus-service status failed", e);
  }
}

async function installFocusService() {
  focusServiceBusy.value = "install";
  focusServiceError.value = "";
  try {
    const r = await window.kepler.focusService.install();
    if (!r.ok) focusServiceError.value = r.error ?? "Установка не удалась";
    await refreshFocusServiceStatus();
  } finally {
    focusServiceBusy.value = "";
  }
}

async function uninstallFocusService() {
  focusServiceBusy.value = "uninstall";
  focusServiceError.value = "";
  try {
    const r = await window.kepler.focusService.uninstall();
    if (!r.ok) focusServiceError.value = r.error ?? "Удаление не удалось";
    await refreshFocusServiceStatus();
  } finally {
    focusServiceBusy.value = "";
  }
}
const focusEditing = ref<boolean>(false);
const focusEditingId = ref<string | null>(null);
const focusDraftName = ref<string>("");
const focusDraftDomains = ref<string>("");
const focusDraftIcon = ref<string>("");
const focusDraftError = ref<string>("");
const focusDragOver = ref<boolean>(false);

// @-mention autocomplete state
const mentionOpen = ref<boolean>(false);
const mentionQuery = ref<string>("");
const mentionAnchor = ref<number>(0); // index of `@` in textarea
const mentionTextareaRef = ref<HTMLTextAreaElement | null>(null);
const mentionHighlight = ref<number>(0);

function isUnknownOperationError(err: unknown): boolean {
  const msg = (err as Error)?.message ?? String(err);
  return /unknown operation|unknown_operation|not.?found/i.test(msg);
}

async function focusRequest<T>(op: string, params?: Record<string, unknown>): Promise<T | null> {
  try {
    const r = await window.kepler.ark.request<T>(op, params);
    return r;
  } catch (e) {
    if (isUnknownOperationError(e)) {
      focusBackendMissing.value = true;
      return null;
    }
    throw e;
  }
}

async function loadBlocklists() {
  focusLoading.value = true;
  focusError.value = "";
  try {
    const r = await focusRequest<{ blocklists: FocusBlocklist[] }>("focus.list_blocklists");
    focusBlocklists.value = r?.blocklists ?? [];
  } catch (e) {
    focusError.value = (e as Error).message;
    focusBlocklists.value = [];
  } finally {
    focusLoading.value = false;
  }
}

async function loadActiveState() {
  try {
    const r = await focusRequest<FocusActiveState>("focus.get_active_state");
    focusActive.value = r ?? { active: false };
  } catch (e) {
    console.warn("focus.get_active_state failed", e);
  }
}

/**
 * Парсит текст blocklist textarea в `{ domains, kind, invalid }`.
 *
 * - Строки начинающиеся с `#` — комментарии, игнорируются.
 * - Строки начинающиеся с `@` — references на другой blocklist (валидируем
 *   что это `@<id>`). Их наличие → kind="raw".
 * - Остальное — должно быть валидным доменом.
 */
function parseRawList(raw: string): {
  domains: string[];
  kind: "domains" | "raw";
  invalid: string[];
} {
  const lines = raw.split("\n");
  const valid: string[] = [];
  const invalid: string[] = [];
  let hasReferences = false;
  for (const line of lines) {
    const stripped = line.replace(/#.*$/, "").trim();
    if (!stripped) continue;
    if (stripped.startsWith("@")) {
      const refId = stripped.slice(1).trim();
      if (!refId) {
        invalid.push(stripped);
        continue;
      }
      hasReferences = true;
      valid.push(`@${refId}`);
    } else if (DOMAIN_PATTERN.test(stripped)) {
      valid.push(stripped.toLowerCase());
    } else {
      invalid.push(stripped);
    }
  }
  return {
    domains: valid,
    kind: hasReferences ? "raw" : "domains",
    invalid,
  };
}

const focusDraftParsed = computed(() => parseRawList(focusDraftDomains.value));

// Кандидаты для @-mention dropdown.
const mentionCandidates = computed<FocusBlocklist[]>(() => {
  const q = mentionQuery.value.toLowerCase();
  return focusBlocklists.value
    .filter((b) => b.id !== focusEditingId.value)
    .filter((b) => !q || b.id.toLowerCase().includes(q) || b.name.toLowerCase().includes(q))
    .slice(0, 6);
});

function openCreateBlocklist() {
  focusEditing.value = true;
  focusEditingId.value = null;
  focusDraftName.value = "";
  focusDraftDomains.value = "";
  focusDraftIcon.value = "";
  focusDraftError.value = "";
}

function openEditBlocklist(bl: FocusBlocklist) {
  focusEditing.value = true;
  focusEditingId.value = bl.id;
  focusDraftName.value = bl.name;
  focusDraftDomains.value = bl.domains.join("\n");
  focusDraftIcon.value = bl.icon ?? "";
  focusDraftError.value = "";
}

function cancelCreateBlocklist() {
  focusEditing.value = false;
  focusEditingId.value = null;
  focusDraftError.value = "";
  mentionOpen.value = false;
}

async function onCreateBlocklist() {
  const name = focusDraftName.value.trim();
  if (!name) {
    focusDraftError.value = "Укажи название блок-листа";
    return;
  }
  const { domains, kind, invalid } = focusDraftParsed.value;
  if (domains.length === 0) {
    focusDraftError.value = "Добавь хотя бы один домен или @ссылку";
    return;
  }
  if (invalid.length > 0) {
    focusDraftError.value = `Невалидных строк: ${invalid.length}. Исправь или удали их`;
    return;
  }
  focusBusy.value = "__create__";
  focusDraftError.value = "";
  try {
    await focusRequest<FocusBlocklist>("focus.upsert_blocklist", {
      id: focusEditingId.value ?? undefined,
      name,
      domains,
      kind,
      icon: focusDraftIcon.value,
    });
    focusEditing.value = false;
    focusEditingId.value = null;
    await loadBlocklists();
  } catch (e) {
    focusDraftError.value = (e as Error).message;
  } finally {
    focusBusy.value = "";
  }
}

// --- @-mention autocomplete ---

function onDomainsInput(e: Event) {
  const ta = e.target as HTMLTextAreaElement;
  const pos = ta.selectionStart;
  const text = ta.value;
  // Найти ближайший `@` слева от cursor'а, без whitespace между.
  let i = pos - 1;
  let at = -1;
  while (i >= 0) {
    const ch = text[i];
    if (ch === "@") {
      at = i;
      break;
    }
    if (ch === " " || ch === "\n" || ch === "\t") break;
    i--;
  }
  if (at >= 0 && (at === 0 || /[\s\n]/.test(text[at - 1] ?? ""))) {
    mentionOpen.value = true;
    mentionAnchor.value = at;
    mentionQuery.value = text.slice(at + 1, pos);
    mentionHighlight.value = 0;
  } else {
    mentionOpen.value = false;
  }
}

function insertMention(bl: FocusBlocklist) {
  const ta = mentionTextareaRef.value;
  if (!ta) return;
  const text = focusDraftDomains.value;
  const before = text.slice(0, mentionAnchor.value);
  const after = text.slice(ta.selectionStart);
  const inserted = `@${bl.id}`;
  focusDraftDomains.value = before + inserted + after;
  mentionOpen.value = false;
  // Restore cursor after the inserted token.
  void Promise.resolve().then(() => {
    const newPos = before.length + inserted.length;
    ta.focus();
    ta.setSelectionRange(newPos, newPos);
  });
}

function onMentionKey(e: KeyboardEvent) {
  if (!mentionOpen.value) return;
  const list = mentionCandidates.value;
  if (e.key === "ArrowDown") {
    e.preventDefault();
    mentionHighlight.value = (mentionHighlight.value + 1) % Math.max(list.length, 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    mentionHighlight.value = (mentionHighlight.value - 1 + list.length) % Math.max(list.length, 1);
  } else if (e.key === "Enter") {
    if (list.length > 0) {
      e.preventDefault();
      insertMention(list[mentionHighlight.value]!);
    }
  } else if (e.key === "Escape") {
    e.preventDefault();
    mentionOpen.value = false;
  }
}

async function onDeleteBlocklist(id: string) {
  if (focusBusy.value) return;
  focusBusy.value = id;
  focusError.value = "";
  try {
    await focusRequest<{ ok: boolean }>("focus.delete_blocklist", { id });
    if (focusActive.value.blocklist_id === id) {
      focusActive.value = { active: false };
    }
    await loadBlocklists();
  } catch (e) {
    focusError.value = (e as Error).message;
  } finally {
    focusBusy.value = "";
  }
}

async function onDeactivate() {
  focusBusy.value = "__deactivate__";
  focusError.value = "";
  try {
    await focusRequest<{ ok: boolean }>("focus.set_active_state", {
      active: false,
    });
    focusActive.value = { active: false };
  } catch (e) {
    focusError.value = (e as Error).message;
  } finally {
    focusBusy.value = "";
  }
}

async function onActivate(id: string) {
  focusBusy.value = id;
  focusError.value = "";
  try {
    await focusRequest<{ ok: boolean }>("focus.set_active_state", {
      active: true,
      blocklist_id: id,
    });
    await loadActiveState();
  } catch (e) {
    focusError.value = (e as Error).message;
  } finally {
    focusBusy.value = "";
  }
}

function onDraftDragOver(e: DragEvent) {
  e.preventDefault();
  focusDragOver.value = true;
}

function onDraftDragLeave() {
  focusDragOver.value = false;
}

async function onDraftDrop(e: DragEvent) {
  e.preventDefault();
  focusDragOver.value = false;
  const file = e.dataTransfer?.files?.[0];
  if (!file) return;
  if (!/\.txt$/i.test(file.name)) {
    focusDraftError.value = "Поддерживаются только .txt файлы";
    return;
  }
  try {
    const text = await file.text();
    focusDraftDomains.value = focusDraftDomains.value
      ? `${focusDraftDomains.value}\n${text}`
      : text;
    if (!focusDraftName.value) {
      focusDraftName.value = file.name.replace(/\.txt$/i, "");
    }
  } catch (err) {
    focusDraftError.value = `Не удалось прочитать файл: ${(err as Error).message}`;
  }
}

const focusActiveBlocklist = computed<FocusBlocklist | undefined>(() => {
  if (!focusActive.value.active || !focusActive.value.blocklist_id) return undefined;
  return focusBlocklists.value.find((b) => b.id === focusActive.value.blocklist_id);
});

function onClose() {
  void window.kepler.settings.close();
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    onClose();
  }
}

function loadTabData(t: Tab) {
  if (t === "extensions") {
    void loadExtensions();
    if (!catalog.value) void loadCatalog();
  }
  if (t === "file-search") {
    void loadFileSearchSettings();
  }
  if (t === "export") {
    void loadExportConverters();
    exportHistory.value = loadExportHistory();
  }
  if (t === "focus") {
    focusBackendMissing.value = false;
    void loadBlocklists();
    void loadActiveState();
    void refreshFocusServiceStatus();
  }
  if (t === "security" || t === "dictation" || t === "secrets") {
    void loadDictationConfig();
  }
  if (t === "dictation") {
    void loadDictationStats();
    void loadDictationMicrophones();
  }
}

function selectTab(t: Tab) {
  tab.value = t;
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

async function onInstallNew(c: MarketplaceExtension) {
  if (installingId.value) return;
  installingId.value = c.id;
  marketError.value = "";
  try {
    await window.kepler.extension.installFromUrl(c.downloadUrl, c.sha256);
    await loadExtensions();
  } catch (e) {
    marketError.value = `${c.id}: ${(e as Error).message}`;
  } finally {
    installingId.value = "";
  }
}

const availableInCatalog = computed<MarketplaceExtension[]>(() => {
  if (!catalog.value) return [];
  const installedIds = new Set(installed.value.map((i) => i.id));
  return catalog.value.extensions.filter((c) => !installedIds.has(c.id));
});

// --- Dictation (Phase 1, Groq) ----------------------------------------------

type DnsKind = "system" | "cloudflare_doh" | "google_doh" | "custom_doh";

interface DictationConfigData {
  hotkey: string;
  triggerMode: "toggle" | "push_to_talk";
  language: string;
  injectMode: "auto_paste" | "clipboard_only";
  networkProfile: { kind: DnsKind; url?: string };
  httpProxy: string | null;
  transcriptionPrompt: string;
  provider: string;
  model: string;
  microphoneDeviceId: string | null;
}

interface DictationStatsData {
  totalWords: number;
  totalRecordSeconds: number;
  totalSessions: number;
  wpm: number;
  timeSavedSeconds: number;
}

const DEFAULT_DICTATION_CFG: DictationConfigData = {
  hotkey: "Ctrl+Shift+;",
  triggerMode: "toggle",
  language: "ru",
  injectMode: "auto_paste",
  networkProfile: { kind: "system" },
  httpProxy: null,
  transcriptionPrompt: "",
  provider: "groq",
  model: "whisper-large-v3-turbo",
  microphoneDeviceId: null,
};

const DEFAULT_DICTATION_STATS: DictationStatsData = {
  totalWords: 0,
  totalRecordSeconds: 0,
  totalSessions: 0,
  wpm: 0,
  timeSavedSeconds: 0,
};

const dictationConfig = ref<DictationConfigData>({ ...DEFAULT_DICTATION_CFG });
const dictationStats = ref<DictationStatsData>({ ...DEFAULT_DICTATION_STATS });
const dictationHasApiKey = ref(false);
const dictationApiKeyInput = ref("");
const dictationApiKeyBusy = ref(false);
const dictationApiKeyMsg = ref("");
const dictationCustomDohUrl = ref("");
const dictationProxyInput = ref("");
const dictationConnTestBusy = ref(false);
const dictationConnTestResult = ref<string>("");
const dictationMicDevices = ref<{ deviceId: string; label: string }[]>([]);
const dictationMicError = ref<string>("");

const dnsProfileOptions = [
  {
    value: "system",
    label: "Системный DNS",
    description: "OS resolver. Самый совместимый, но в РФ Groq обычно блокирован.",
  },
  {
    value: "cloudflare_doh",
    label: "Cloudflare DoH (1.1.1.1)",
    description: "Обходит DNS poisoning. Безопасный default.",
  },
  { value: "google_doh", label: "Google DoH (8.8.8.8)", description: "Альтернатива Cloudflare." },
  {
    value: "custom_doh",
    label: "Свой DoH URL",
    description: "Укажите endpoint в формате https://example/dns-query.",
  },
] as const;

const dictationTriggerOptions = [
  {
    value: "toggle",
    label: "Toggle (двойное нажатие)",
    description: "Первое нажатие — старт, второе — отправка.",
  },
  {
    value: "push_to_talk",
    label: "Push-to-talk (удерживать)",
    description: "Удерживайте клавишу пока говорите.",
  },
] as const;

const dictationInjectOptions = [
  {
    value: "auto_paste",
    label: "Auto-paste",
    description: "Симулирует Ctrl+V и восстанавливает буфер.",
  },
  {
    value: "clipboard_only",
    label: "Только в буфер обмена",
    description: "Текст записывается в буфер, Ctrl+V — вручную.",
  },
] as const;

// Whisper supports 90+ языков. Включаем самые востребованные + `auto`
// (отсутствие language → авто-определение Whisper'ом).
const dictationLanguageOptions = [
  { value: "auto", label: "Авто" },
  { value: "ru", label: "Русский" },
  { value: "en", label: "English" },
  { value: "uk", label: "Українська" },
  { value: "be", label: "Беларуская" },
  { value: "de", label: "Deutsch" },
  { value: "fr", label: "Français" },
  { value: "es", label: "Español" },
  { value: "it", label: "Italiano" },
  { value: "pt", label: "Português" },
  { value: "pl", label: "Polski" },
  { value: "tr", label: "Türkçe" },
  { value: "ja", label: "日本語" },
  { value: "ko", label: "한국어" },
  { value: "zh", label: "中文" },
  { value: "ar", label: "العربية" },
  { value: "he", label: "עברית" },
  { value: "hi", label: "हिन्दी" },
  { value: "nl", label: "Nederlands" },
  { value: "sv", label: "Svenska" },
  { value: "fi", label: "Suomi" },
  { value: "cs", label: "Čeština" },
  { value: "el", label: "Ελληνικά" },
] as const;

const dictationProviderOptions = [{ value: "groq", label: "Groq Cloud" }] as const;

async function onDictationProviderChange(v: string) {
  await patchDictationConfig({ provider: v });
}

const dictationProviderDescription = computed(() =>
  dictationHasApiKey.value
    ? "Ключ установлен в разделе «Секреты»."
    : "Не задан API-ключ — добавьте его в разделе «Секреты».",
);

async function loadDictationConfig() {
  try {
    const resp = (await window.kepler.ark.request("dictation.get_config", {})) as {
      config?: Partial<DictationConfigData>;
      hasApiKey?: boolean;
    };
    if (resp.config) {
      dictationConfig.value = {
        ...DEFAULT_DICTATION_CFG,
        ...resp.config,
        networkProfile: resp.config.networkProfile ?? { kind: "system" },
      };
      if (dictationConfig.value.networkProfile.kind === "custom_doh") {
        dictationCustomDohUrl.value = dictationConfig.value.networkProfile.url ?? "";
      }
      dictationProxyInput.value = dictationConfig.value.httpProxy ?? "";
    }
    dictationHasApiKey.value = resp.hasApiKey ?? false;
  } catch (e) {
    console.error("[settings] loadDictationConfig failed:", e);
  }
}

async function loadDictationStats() {
  try {
    const resp = (await window.kepler.ark.request(
      "dictation.get_stats",
      {},
    )) as Partial<DictationStatsData>;
    dictationStats.value = { ...DEFAULT_DICTATION_STATS, ...resp };
  } catch (e) {
    console.error("[settings] loadDictationStats failed:", e);
  }
}

// Enumerate микрофоны через Web API. Labels пустые пока пользователь не дал
// permission на mic. Поэтому запрашиваем permission shot'ом (мини-stream
// сразу останавливаем) и затем enumerate'им повторно.
async function loadDictationMicrophones() {
  dictationMicError.value = "";
  try {
    // 1. shot permission (если ещё не давал) — иначе labels пустые.
    try {
      const probe = await navigator.mediaDevices.getUserMedia({ audio: true });
      for (const t of probe.getTracks()) t.stop();
    } catch (permErr) {
      dictationMicError.value =
        "Нет доступа к микрофону — выберите устройство после первого запуска записи.";
      console.warn("[settings] mic permission probe failed:", permErr);
    }
    // 2. enumerate.
    const devices = await navigator.mediaDevices.enumerateDevices();
    dictationMicDevices.value = devices
      .filter((d) => d.kind === "audioinput")
      .map((d, i) => ({
        deviceId: d.deviceId,
        label: d.label || `Микрофон ${i + 1}`,
      }));
  } catch (e) {
    console.error("[settings] loadDictationMicrophones failed:", e);
    dictationMicError.value = `Ошибка: ${(e as Error).message}`;
  }
}

async function onDictationMicChange(v: string) {
  // "" / "default" → null (системный default).
  const next = v && v !== "default" ? v : null;
  await patchDictationConfig({ microphoneDeviceId: next });
}

function formatTimeSaved(sec: number): { value: string; unit: string } {
  if (sec < 60) return { value: String(Math.max(0, Math.round(sec))), unit: "сек" };
  if (sec < 3600) return { value: String(Math.round(sec / 60)), unit: "мин" };
  return { value: (sec / 3600).toFixed(1), unit: "ч" };
}

function formatTotalWords(n: number): { value: string; unit: string } {
  if (n < 1000) return { value: String(n), unit: "" };
  if (n < 1_000_000) return { value: (n / 1000).toFixed(1), unit: "k" };
  return { value: (n / 1_000_000).toFixed(1), unit: "M" };
}

const statsCards = computed(() => {
  const wpm = Math.round(dictationStats.value.wpm);
  const saved = formatTimeSaved(dictationStats.value.timeSavedSeconds);
  const total = formatTotalWords(dictationStats.value.totalWords);
  return [
    { key: "wpm", label: "WPM", value: String(wpm), unit: "" },
    { key: "saved", label: "Сэкономлено", value: saved.value, unit: saved.unit },
    { key: "words", label: "Всего слов", value: total.value, unit: total.unit },
  ];
});

const dictationMicOptions = computed(() => {
  const opts: { value: string; label: string }[] = [
    { value: "default", label: "Системный по умолчанию" },
  ];
  for (const d of dictationMicDevices.value) {
    // default ID часто тоже идёт в enumerateDevices — но трактуем как
    // explicit choice пользователя, поэтому добавляем как есть.
    opts.push({ value: d.deviceId, label: d.label });
  }
  return opts;
});

async function patchDictationConfig(patch: Record<string, unknown>) {
  try {
    await window.kepler.ark.request("dictation.update_config", patch);
    await loadDictationConfig();
  } catch (e) {
    console.error("[settings] update_config failed:", e);
  }
}

async function onDictationLanguageChange(v: string) {
  await patchDictationConfig({ language: v });
}
async function onDictationInjectModeChange(v: "auto_paste" | "clipboard_only") {
  await patchDictationConfig({ injectMode: v });
}
async function onDictationHotkeyCapture(acc: string) {
  if (!acc) return;
  await patchDictationConfig({ hotkey: acc });
}

// External capture flow для системных shortcut'ов (Win+H и т.п.):
// HotkeyCapture зовёт capture-start → мы говорим backend'у активировать
// hook в capture mode → hook эмитит `dictation_capture_key` с vk +
// модификаторами → конвертируем в accelerator string → передаём обратно
// через pendingAccelerator ref. Также подписываемся на `_cancelled` (Esc).
const dictationCaptureAccelerator = ref<string | null>(null);
const dictationCaptureCancelTick = ref(0);
let dictationCaptureUnsubscribe: (() => void) | null = null;

function vkToKeyName(vk: number): string {
  // ASCII A-Z (0x41-0x5A) и 0-9 (0x30-0x39).
  if ((vk >= 0x41 && vk <= 0x5a) || (vk >= 0x30 && vk <= 0x39)) {
    return String.fromCharCode(vk);
  }
  // F1-F24.
  if (vk >= 0x70 && vk <= 0x87) return `F${vk - 0x6f}`;
  // OEM punctuation.
  const oem: Record<number, string> = {
    0xba: ";",
    0xbb: "+",
    0xbc: ",",
    0xbd: "-",
    0xbe: ".",
    0xbf: "/",
    0xc0: "`",
    0xdb: "[",
    0xdc: "\\",
    0xdd: "]",
    0xde: "'",
  };
  if (oem[vk]) return oem[vk];
  // Named keys.
  const named: Record<number, string> = {
    0x08: "Backspace",
    0x09: "Tab",
    0x0d: "Enter",
    0x20: "Space",
    0x21: "PageUp",
    0x22: "PageDown",
    0x23: "End",
    0x24: "Home",
    0x25: "Left",
    0x26: "Up",
    0x27: "Right",
    0x28: "Down",
    0x2d: "Insert",
    0x2e: "Delete",
  };
  return named[vk] ?? "";
}

function buildAccelerator(payload: {
  vk: number;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  win?: boolean;
}): string {
  const parts: string[] = [];
  if (payload.ctrl) parts.push("Ctrl");
  if (payload.alt) parts.push("Alt");
  if (payload.shift) parts.push("Shift");
  if (payload.win) parts.push("Super");
  const key = vkToKeyName(payload.vk);
  if (!key) return "";
  parts.push(key);
  return parts.join("+");
}

function onDictationCaptureStart() {
  dictationCaptureAccelerator.value = null;
  // Подписываемся ДО begin_hotkey_capture, иначе можем упустить первое
  // нажатие если юзер крайне быстрый.
  if (!dictationCaptureUnsubscribe) {
    dictationCaptureUnsubscribe = window.kepler.dictation.onCaptureEvent((e) => {
      if (e.event === "dictation_capture_key") {
        const acc = buildAccelerator({
          vk: e.vk as number,
          ctrl: e.ctrl as boolean,
          shift: e.shift as boolean,
          alt: e.alt as boolean,
          win: e.win as boolean,
        });
        if (acc) dictationCaptureAccelerator.value = acc;
      } else if (e.event === "dictation_capture_cancelled") {
        dictationCaptureCancelTick.value++;
      }
    });
  }
  void window.kepler.ark.request("dictation.begin_hotkey_capture", {});
}

function onDictationCaptureEnd() {
  void window.kepler.ark.request("dictation.end_hotkey_capture", {});
  dictationCaptureUnsubscribe?.();
  dictationCaptureUnsubscribe = null;
}
async function onDictationDnsKindChange(kind: DnsKind) {
  const profile: { kind: DnsKind; url?: string } = { kind };
  if (kind === "custom_doh") profile.url = dictationCustomDohUrl.value.trim();
  await patchDictationConfig({ networkProfile: profile });
}
async function onDictationCustomDohBlur() {
  if (dictationConfig.value.networkProfile.kind !== "custom_doh") return;
  await patchDictationConfig({
    networkProfile: { kind: "custom_doh", url: dictationCustomDohUrl.value.trim() },
  });
}

async function onDictationSaveApiKey() {
  const key = dictationApiKeyInput.value.trim();
  if (!key) return;
  dictationApiKeyBusy.value = true;
  dictationApiKeyMsg.value = "";
  try {
    await window.kepler.ark.request("dictation.set_api_key", { key });
    dictationApiKeyInput.value = "";
    dictationApiKeyMsg.value = "Сохранено в Windows Credential Manager";
    await loadDictationConfig();
  } catch (e) {
    dictationApiKeyMsg.value = `Ошибка: ${(e as Error).message}`;
  } finally {
    dictationApiKeyBusy.value = false;
  }
}

async function onDictationClearApiKey() {
  dictationApiKeyBusy.value = true;
  dictationApiKeyMsg.value = "";
  try {
    await window.kepler.ark.request("dictation.clear_api_key", {});
    dictationApiKeyMsg.value = "Ключ удалён";
    await loadDictationConfig();
  } catch (e) {
    dictationApiKeyMsg.value = `Ошибка: ${(e as Error).message}`;
  } finally {
    dictationApiKeyBusy.value = false;
  }
}

async function onDictationTestConnectivity() {
  dictationConnTestBusy.value = true;
  dictationConnTestResult.value = "";
  try {
    const resp = (await window.kepler.ark.request("dictation.test_connectivity", {})) as {
      ok?: boolean;
      status?: number;
      latencyMs?: number;
    };
    if (resp.ok) {
      dictationConnTestResult.value = `OK · ${resp.status ?? "—"} · ${resp.latencyMs ?? "?"}ms`;
    } else {
      dictationConnTestResult.value = "Не удалось";
    }
  } catch (e) {
    dictationConnTestResult.value = `Ошибка: ${(e as Error).message}`;
  } finally {
    dictationConnTestBusy.value = false;
  }
}

async function onDictationTriggerModeChange(v: "toggle" | "push_to_talk") {
  await patchDictationConfig({ triggerMode: v });
}

async function onDictationProxyBlur() {
  const v = dictationProxyInput.value.trim();
  await patchDictationConfig({ httpProxy: v === "" ? null : v });
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
  unsubscribeFocusServiceStatus = window.kepler.focusService.onStatusChanged(() => {
    void refreshFocusServiceStatus();
  });
});

watch(activeTab, (nextTab, previousTab) => {
  if (!nextTab || nextTab === previousTab) return;
  loadTabData(nextTab);
});

onBeforeUnmount(() => {
  unsubscribeUpdateState?.();
  unsubscribeUpdateState = null;
  unsubscribeFocusServiceStatus?.();
  unsubscribeFocusServiceStatus = null;
  clearFileSearchPoll();
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
                  @update:modelValue="onLauncherHotkeyChange"
                />
                <Button variant="ghost" size="sm" @click="resetHotkey">Сброс</Button>
              </div>
            </div>

            <div class="row">
              <div class="row-label">
                <div class="label">Автозапуск с Windows</div>
                <div class="hint">
                  <template v-if="autostartAllowed">Запускать Kepler при входе в систему</template>
                  <template v-else
                    >Доступно только в установленной версии (не в dev-сборке)</template
                  >
                </div>
                <div v-if="autostartError" class="error">{{ autostartError }}</div>
              </div>
              <label class="toggle" :class="{ disabled: !autostartAllowed }">
                <input
                  type="checkbox"
                  :checked="autostart"
                  :disabled="!autostartAllowed"
                  @change="onToggleAutostart"
                />
                <span class="track"><span class="thumb" /></span>
              </label>
            </div>

            <div class="row">
              <div class="row-label">
                <div class="label">Показывать в трее</div>
                <div class="hint">Оставлять значок Kepler в системном трее</div>
              </div>
              <label class="toggle">
                <input type="checkbox" :checked="trayIcon" @change="onToggleTrayIcon" />
                <span class="track"><span class="thumb" /></span>
              </label>
            </div>
          </div>
        </template>

        <template v-else-if="activeTab === 'about'">
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

            <div class="advanced-page__body rows">
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
          </section>
        </template>

        <template v-else-if="activeTab === 'debug'">
          <div class="rows kosmos-scroll">
            <div class="row">
              <div class="row-label">
                <div class="label">Developer mode</div>
                <div class="hint">
                  Hot reload extension'ов через Vite + F12 для DevTools. Перезапусти extension чтобы
                  применить.
                </div>
              </div>
              <label class="toggle">
                <input type="checkbox" :checked="developerMode" @change="onToggleDeveloperMode" />
                <span class="track"><span class="thumb" /></span>
              </label>
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
                  @change="onLauncherStateTtlChange"
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

            <div class="row" v-if="backend.lockFilePath">
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
                    {{ crashFiles.length }} {{ crashFiles.length === 1 ? "файл" : "файла(-ов)" }} в
                    папке отчётов.
                  </template>
                </div>
              </div>
              <div class="row-actions">
                <button type="button" class="btn ghost" @click="onOpenCrashesFolder">
                  Открыть папку
                </button>
                <button
                  type="button"
                  class="btn ghost"
                  :disabled="crashFiles.length === 0"
                  @click="onClearCrashes"
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
                  <template v-if="bundleSavedPath">
                    Сохранён: <code>{{ bundleSavedPath }}</code>
                  </template>
                  <template v-else-if="bundling">Собираю отчёт…</template>
                  <template v-else>Логи + crash-reports + версии.</template>
                </div>
              </div>
              <div class="row-actions">
                <button type="button" class="btn ghost" :disabled="bundling" @click="onBundleSave">
                  Создать отчёт
                </button>
                <button type="button" class="btn ghost" @click="onOpenLogsFolder">
                  Открыть logs/
                </button>
              </div>
            </div>
            <div v-if="bundleError" class="error-banner">{{ bundleError }}</div>
          </div>
        </template>

        <!-- Безопасность — сеть для AI-провайдеров (DNS resolver). -->
        <template v-else-if="activeTab === 'security'">
          <div class="security-page kosmos-scroll">
            <p class="security-page__intro">
              Какой DNS-резолвер и прокси использовать для запросов к Groq и другим AI. В РФ Groq
              часто блокируется через DNS poisoning — DoH (DNS-over-HTTPS) это обходит без VPN. SNI
              / IP-блок DoH не лечит — для этого нужен прокси ниже.
            </p>
            <SettingsList>
              <SettingsRow
                title="DNS для AI-провайдеров"
                description="Резолвер только для dictation HTTP. Не влияет на остальной сетевой стек."
              >
                <template #control>
                  <RadioGroup
                    :model-value="dictationConfig.networkProfile.kind"
                    :options="dnsProfileOptions"
                    name="dictation-dns"
                    @update:modelValue="onDictationDnsKindChange"
                  />
                </template>
              </SettingsRow>
              <SettingsRow
                v-if="dictationConfig.networkProfile.kind === 'custom_doh'"
                title="Свой DoH URL"
                description="Endpoint в формате https://example/dns-query."
              >
                <template #control>
                  <TextInput
                    v-model="dictationCustomDohUrl"
                    placeholder="https://comss.dns.controld.com/dns-query"
                    @blur="onDictationCustomDohBlur"
                  />
                </template>
              </SettingsRow>
              <SettingsRow
                title="HTTP / SOCKS proxy"
                description="Опционально. Если DoH недостаточно (SNI / IP блок). http://, https://, socks5://host:port. Пусто — без proxy."
              >
                <template #control>
                  <TextInput
                    v-model="dictationProxyInput"
                    placeholder="socks5://127.0.0.1:1080"
                    @blur="onDictationProxyBlur"
                  />
                </template>
              </SettingsRow>
              <SettingsRow
                title="Проверить соединение"
                :description="
                  dictationConnTestResult ||
                  'HEAD-запрос к api.groq.com через выбранный DNS + proxy.'
                "
              >
                <template #control>
                  <Button
                    variant="ghost"
                    :loading="dictationConnTestBusy"
                    :disabled="dictationConnTestBusy"
                    @click="onDictationTestConnectivity"
                  >
                    {{ dictationConnTestBusy ? "Проверяю…" : "Проверить" }}
                  </Button>
                </template>
              </SettingsRow>
            </SettingsList>
          </div>
        </template>

        <!-- Секреты — API-ключи для AI-провайдеров. -->
        <template v-else-if="activeTab === 'secrets'">
          <div class="security-page kosmos-scroll">
            <p class="security-page__intro">
              API-ключи хранятся в Windows Credential Manager — изолированно от файлов конфигурации,
              не попадают в backup'ы и логи. Используются функциями, которым нужен соответствующий
              провайдер (сейчас — только диктация Groq).
            </p>
            <SettingsList>
              <SettingsRow title="Groq">
                <template #control>
                  <div class="control-stack">
                    <TextInput
                      v-model="dictationApiKeyInput"
                      type="password"
                      autocomplete="new-password"
                      :placeholder="dictationHasApiKey ? '••••••••' : 'gsk_...'"
                      :block="false"
                    />
                    <Button
                      variant="primary"
                      size="sm"
                      :loading="dictationApiKeyBusy"
                      :disabled="dictationApiKeyBusy || !dictationApiKeyInput.trim()"
                      @click="onDictationSaveApiKey"
                    >
                      Сохранить
                    </Button>
                    <Button
                      v-if="dictationHasApiKey"
                      variant="ghost"
                      size="sm"
                      :disabled="dictationApiKeyBusy"
                      @click="onDictationClearApiKey"
                    >
                      Очистить
                    </Button>
                  </div>
                </template>
              </SettingsRow>
            </SettingsList>
          </div>
        </template>

        <!-- Диктация (продвинутые) — hotkey, язык, inject, API key. -->
        <template v-else-if="activeTab === 'dictation'">
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
              <div class="stats-header">
                <span class="stats-header__title">Статистика</span>
              </div>
              <div class="stats-cards">
                <div v-for="card in statsCards" :key="card.key" class="stats-card">
                  <div class="stats-card__label">{{ card.label }}</div>
                  <div class="stats-card__value">
                    <span class="stats-card__number">{{ card.value }}</span>
                    <span v-if="card.unit" class="stats-card__unit">{{ card.unit }}</span>
                  </div>
                </div>
              </div>

              <div class="stats-header">
                <span class="stats-header__title">Микрофон</span>
              </div>
              <SettingsList>
                <SettingsDropdownRow
                  title="Устройство"
                  :description="
                    dictationMicError ||
                    'Если устройство не доступно во время записи — будет fallback на системный default.'
                  "
                  :model-value="dictationConfig.microphoneDeviceId ?? 'default'"
                  :options="dictationMicOptions"
                  @update:modelValue="onDictationMicChange"
                />
              </SettingsList>

              <div class="stats-header">
                <span class="stats-header__title">Основное</span>
              </div>
              <SettingsList>
                <SettingsDropdownRow
                  title="Язык"
                  description="Подсказка для Whisper. «Авто» — автоопределение."
                  :model-value="dictationConfig.language"
                  :options="dictationLanguageOptions"
                  @update:modelValue="onDictationLanguageChange"
                />
                <SettingsRow title="Горячая клавиша">
                  <template #control>
                    <HotkeyCapture
                      :model-value="dictationConfig.hotkey"
                      capture-prompt="Нажми сочетание…"
                      external-capture
                      :pending-accelerator="dictationCaptureAccelerator"
                      :pending-cancel="dictationCaptureCancelTick"
                      @capture-start="onDictationCaptureStart"
                      @capture-end="onDictationCaptureEnd"
                      @update:modelValue="onDictationHotkeyCapture"
                    />
                  </template>
                </SettingsRow>
                <SettingsDropdownRow
                  title="Режим триггера"
                  description="Как срабатывает горячая клавиша."
                  :model-value="dictationConfig.triggerMode"
                  :options="dictationTriggerOptions"
                  @update:modelValue="onDictationTriggerModeChange"
                />
                <SettingsDropdownRow
                  title="Вставка"
                  description="Auto-paste симулирует Ctrl+V и восстанавливает буфер. Clipboard — только записать текст, вы жмёте Ctrl+V сами."
                  :model-value="dictationConfig.injectMode"
                  :options="dictationInjectOptions"
                  @update:modelValue="onDictationInjectModeChange"
                />
                <SettingsDropdownRow
                  title="Поставщик"
                  :description="dictationProviderDescription"
                  :model-value="dictationConfig.provider"
                  :options="dictationProviderOptions"
                  @update:modelValue="onDictationProviderChange"
                />
              </SettingsList>
            </div>
          </section>
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
                  <label class="toggle">
                    <input type="checkbox" :checked="usageTracker" @change="onToggleUsageTracker" />
                    <span class="track"><span class="thumb" /></span>
                  </label>
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

            <div class="advanced-page__body rows">
              <div v-if="fileSearchError" class="error-banner">{{ fileSearchError }}</div>
              <!-- Regression M8 (2026-05-24): show loading state instead of
                   misleading "no folders chosen" while settings load. -->
              <div v-if="fileSearchSettings === null && !fileSearchError" class="hint">
                Загрузка настроек поиска…
              </div>
              <div v-if="fileSearchSettings" class="row file-search-row">
                <div class="row-label file-search-wide">
                  <div class="label">Папки поиска</div>
                  <div class="hint">
                    Kepler индексирует только выбранные папки. По умолчанию это профиль
                    пользователя; большие диски лучше добавлять осознанно.
                  </div>
                  <!-- Regression L5 (2026-05-24): semantic list + button
                       aria-label includes the scope so screen readers don't
                       read "Удалить, Удалить, Удалить..." for N chips. -->
                  <ul class="file-search-list" role="list">
                    <li
                      v-for="root in fileSearchSettings?.roots ?? []"
                      :key="root"
                      class="file-search-chip"
                    >
                      <span class="file-search-chip__icon" aria-hidden="true">
                        <Folder :size="16" :stroke-width="1.75" />
                      </span>
                      <code>{{ root }}</code>
                      <button
                        type="button"
                        class="btn ghost danger"
                        :disabled="fileSearchBusy"
                        :aria-label="`Удалить папку поиска ${root}`"
                        @click="onRemoveFileSearchScope(root)"
                      >
                        Удалить
                      </button>
                    </li>
                    <li v-if="(fileSearchSettings?.roots.length ?? 0) === 0" class="hint">
                      Папки не выбраны — поиск файлов ничего не индексирует.
                    </li>
                  </ul>
                </div>
                <div class="row-actions">
                  <button
                    type="button"
                    class="btn"
                    :disabled="!fileSearchSettings || fileSearchBusy"
                    @click="onAddFileSearchScope"
                  >
                    Добавить…
                  </button>
                </div>
              </div>

              <div v-if="fileSearchSettings" class="row file-search-row">
                <div class="row-label file-search-wide">
                  <div class="label">Шаблоны исключений</div>
                  <div class="hint">
                    Паттерны применяются к имени и пути файла. Примеры:
                    <code>*.tmp</code>, <code>*.log</code>, <code>**/Cache/**</code>.
                  </div>
                  <ul class="file-search-list" role="list">
                    <li
                      v-for="pattern in fileSearchSettings?.ignore_patterns ?? []"
                      :key="pattern"
                      class="file-search-chip"
                    >
                      <code>{{ pattern }}</code>
                      <button
                        type="button"
                        class="btn ghost danger"
                        :disabled="fileSearchBusy"
                        :aria-label="`Удалить шаблон ${pattern}`"
                        @click="onRemoveFileSearchIgnore(pattern)"
                      >
                        Удалить
                      </button>
                    </li>
                    <li v-if="(fileSearchSettings?.ignore_patterns.length ?? 0) === 0" class="hint">
                      Пользовательских шаблонов пока нет.
                    </li>
                  </ul>
                  <div class="file-search-add">
                    <input
                      v-model="fileSearchNewIgnore"
                      class="focus-input"
                      type="text"
                      placeholder="*.tmp"
                      :disabled="fileSearchBusy"
                      @keydown.enter.prevent="onAddFileSearchIgnore"
                    />
                    <button
                      type="button"
                      class="btn"
                      :disabled="fileSearchBusy || fileSearchNewIgnore.trim().length === 0"
                      @click="onAddFileSearchIgnore"
                    >
                      Добавить
                    </button>
                  </div>
                </div>
              </div>

              <div v-if="fileSearchSettings" class="row">
                <div class="row-label">
                  <div class="label">Исключать шумные папки из поиска файлов</div>
                  <div class="hint">
                    Kepler индексирует файлы на локальных дисках и пропускает
                    <code>node_modules</code>, <code>.git</code>, сборки и временные каталоги.
                    Изменение сразу запускает переиндексацию.
                  </div>
                </div>
                <label class="toggle" :class="{ disabled: !fileSearchSettings || fileSearchBusy }">
                  <input
                    type="checkbox"
                    :checked="fileSearchSettings?.exclude_noisy_folders ?? true"
                    :disabled="!fileSearchSettings || fileSearchBusy"
                    @change="onToggleFileSearchNoise"
                  />
                  <span class="track"><span class="thumb" /></span>
                </label>
              </div>

              <div v-if="fileSearchSettings" class="row">
                <div class="row-label">
                  <div class="label">Учитывать <code>.gitignore</code></div>
                  <div class="hint">
                    Включено по умолчанию: Kepler пропускает файлы, которые проект сам считает
                    мусором.
                  </div>
                </div>
                <label class="toggle" :class="{ disabled: !fileSearchSettings || fileSearchBusy }">
                  <input
                    type="checkbox"
                    :checked="fileSearchSettings?.respect_gitignore ?? true"
                    :disabled="!fileSearchSettings || fileSearchBusy"
                    @change="onToggleFileSearchGitignore"
                  />
                  <span class="track"><span class="thumb" /></span>
                </label>
              </div>

              <div v-if="fileSearchSettings" class="row">
                <div class="row-label">
                  <div class="label">Показывать скрытые файлы</div>
                  <div class="hint">
                    По умолчанию выключено, чтобы не засорять результаты dotfiles и системными
                    скрытыми файлами.
                  </div>
                </div>
                <label class="toggle" :class="{ disabled: !fileSearchSettings || fileSearchBusy }">
                  <input
                    type="checkbox"
                    :checked="fileSearchSettings?.include_hidden ?? false"
                    :disabled="!fileSearchSettings || fileSearchBusy"
                    @change="onToggleFileSearchHidden"
                  />
                  <span class="track"><span class="thumb" /></span>
                </label>
              </div>

              <div v-if="fileSearchSettings" class="row">
                <div class="row-label">
                  <div class="label">Ускоренный NTFS-режим</div>
                  <div class="hint">
                    Если включено, Kepler пробует быстрый NTFS/MFT scan для корней дисков. Если
                    service или права недоступны — автоматически падает назад на обычный scan.
                    NTFS-режим не уважает <code>.gitignore</code>: при включённой обработке
                    <code>.gitignore</code> диски сканируются обычным способом.
                  </div>
                  <!-- Regression H10 (2026-05-24): surface actual NTFS status,
                       not just toggle position. -->
                  <div
                    v-if="
                      fileSearchSettings?.ntfs_accelerated &&
                      fileSearchSettings?.ntfs_status &&
                      fileSearchSettings.ntfs_status !== 'disabled' &&
                      fileSearchSettings.ntfs_status !== 'unknown'
                    "
                    class="hint"
                    :class="{
                      'ntfs-status-active': fileSearchSettings.ntfs_status === 'active',
                      'ntfs-status-fallback':
                        fileSearchSettings.ntfs_status === 'fallback' ||
                        fileSearchSettings.ntfs_status === 'unavailable',
                    }"
                  >
                    Статус:
                    <strong v-if="fileSearchSettings.ntfs_status === 'active'"> активен </strong>
                    <strong v-else-if="fileSearchSettings.ntfs_status === 'fallback'">
                      резервный режим
                    </strong>
                    <strong v-else-if="fileSearchSettings.ntfs_status === 'unavailable'">
                      недоступен
                    </strong>
                  </div>
                </div>
                <label class="toggle" :class="{ disabled: !fileSearchSettings || fileSearchBusy }">
                  <input
                    type="checkbox"
                    :checked="fileSearchSettings?.ntfs_accelerated ?? false"
                    :disabled="!fileSearchSettings || fileSearchBusy"
                    @change="onToggleFileSearchNtfs"
                  />
                  <span class="track"><span class="thumb" /></span>
                </label>
              </div>

              <div v-if="fileSearchSettings" class="row">
                <div class="row-label">
                  <div class="label">Переиндексация</div>
                  <div class="hint">Запусти вручную после больших перемещений файлов.</div>
                </div>
                <div class="row-actions">
                  <button
                    type="button"
                    class="btn"
                    :disabled="
                      !fileSearchSettings || fileSearchBusy || fileSearchSettings?.scan_in_progress
                    "
                    @click="onRescanFileSearch"
                  >
                    {{
                      fileSearchBusy || fileSearchSettings?.scan_in_progress
                        ? "Идёт…"
                        : "Переиндексировать"
                    }}
                  </button>
                </div>
              </div>
            </div>
          </section>
        </template>

        <!-- Extensions tab — плоский список установленных. Обновления подтягиваются из catalog.json. -->
        <template v-else-if="activeTab === 'extensions'">
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
              <div v-if="marketError" class="error-banner">{{ marketError }}</div>
              <div v-if="extensionsError" class="error-banner">{{ extensionsError }}</div>

              <div class="ext-list kosmos-scroll">
                <!-- Доступные из marketplace, ещё не установленные -->
                <div v-if="availableInCatalog.length > 0" class="ext-section-title">
                  Доступные расширения
                </div>
                <div v-for="c in availableInCatalog" :key="`catalog-${c.id}`" class="ext-item">
                  <img
                    v-if="c.iconUrl"
                    class="ext-icon"
                    :src="c.iconUrl"
                    alt=""
                    @error="(e) => ((e.target as HTMLImageElement).style.display = 'none')"
                  />
                  <div v-else class="ext-icon ext-icon-fallback">
                    {{ c.name.slice(0, 1) }}
                  </div>
                  <div class="ext-info">
                    <div class="ext-name">{{ c.name }}</div>
                    <div class="ext-meta">
                      <span class="ext-version">v{{ c.version }}</span>
                      <span v-if="c.author" class="ext-author">· {{ c.author }}</span>
                    </div>
                    <div v-if="c.description" class="ext-description">
                      {{ c.description }}
                    </div>
                  </div>
                  <div class="ext-actions">
                    <button
                      type="button"
                      class="btn"
                      :disabled="installingId === c.id"
                      @click="onInstallNew(c)"
                    >
                      <template v-if="installingId === c.id">Установка…</template>
                      <template v-else>Установить</template>
                    </button>
                  </div>
                </div>

                <!-- Установленные -->
                <div
                  v-if="installed.length > 0 && availableInCatalog.length > 0"
                  class="ext-section-title"
                >
                  Установленные
                </div>
                <div v-if="installed.length === 0 && availableInCatalog.length === 0" class="empty">
                  <template v-if="marketLoading">Загрузка каталога…</template>
                  <template v-else>Расширений нет. Каталог пуст или недоступен.</template>
                </div>

                <div v-for="ext in installed" :key="ext.id" class="ext-item">
                  <img v-if="ext.iconDataUri" class="ext-icon" :src="ext.iconDataUri" alt="" />
                  <div v-else class="ext-icon ext-icon-fallback">
                    {{ ext.name.slice(0, 1) }}
                  </div>
                  <div class="ext-info">
                    <div class="ext-name">{{ ext.name }}</div>
                    <div class="ext-meta">
                      <span class="ext-version">v{{ ext.version ?? "—" }}</span>
                      <span v-if="ext.source === 'dev'" class="ext-dev-badge">dev</span>
                      <span v-if="ext.author" class="ext-author">· {{ ext.author }}</span>
                      <span v-if="hasUpdate(ext) && ext.source !== 'dev'" class="ext-author">
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
                    <!-- Dev-source extension'ы (из repo) НЕ имеют update/revert/uninstall —
                 source code управляется git'ом, не Kepler installer'ом. -->
                    <span v-if="ext.source === 'dev'" class="ext-dev-hint">
                      источник: репозиторий
                    </span>
                    <template v-else>
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
                    </template>
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
            </div>
          </section>
        </template>

        <!-- Focus tab — управление блок-листами доменов и активной блокировкой -->
        <template v-else-if="activeTab === 'focus'">
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
              <div v-if="focusBackendMissing" class="error-banner">
                Backend ещё не поддерживает focus.*. Обнови Kepler.
              </div>
              <div v-if="focusError" class="error-banner">{{ focusError }}</div>

              <div class="rows kosmos-scroll">
                <!-- Демон фокус-режима — устранит UAC при каждом включении блокировки -->
                <div class="row focus-service-row">
                  <div class="row-label">
                    <div class="label">Системный демон</div>
                    <div
                      v-if="focusServiceStatus.installed && focusServiceStatus.running"
                      class="hint focus-service-hint-ok"
                    >
                      Установлен и работает — блокировка включается без запроса прав администратора.
                    </div>
                    <div v-else-if="focusServiceStatus.installed" class="hint">
                      Установлен, но не запущен. Перезапусти Windows или нажми «Переустановить».
                    </div>
                    <div v-else class="hint">
                      Без демона Windows запрашивает права администратора при каждом включении
                      блокировки. Установи один раз — и все последующие активации будут без UAC.
                    </div>
                    <div v-if="focusServiceError" class="error">{{ focusServiceError }}</div>
                  </div>
                  <div class="row-actions">
                    <button
                      v-if="!focusServiceStatus.installed"
                      type="button"
                      class="btn primary"
                      :disabled="focusServiceBusy === 'install'"
                      @click="installFocusService"
                    >
                      {{ focusServiceBusy === "install" ? "Установка…" : "Установить" }}
                    </button>
                    <template v-else>
                      <button
                        type="button"
                        class="btn ghost"
                        :disabled="focusServiceBusy === 'install'"
                        @click="installFocusService"
                        title="Переустановить если служба перестала работать"
                      >
                        {{ focusServiceBusy === "install" ? "Установка…" : "Переустановить" }}
                      </button>
                      <button
                        type="button"
                        class="btn ghost danger"
                        :disabled="focusServiceBusy === 'uninstall'"
                        @click="uninstallFocusService"
                      >
                        {{ focusServiceBusy === "uninstall" ? "Удаление…" : "Удалить" }}
                      </button>
                    </template>
                  </div>
                </div>

                <!-- Активная блокировка -->
                <div class="row focus-active-row">
                  <div class="row-label">
                    <div class="label">Активная блокировка</div>
                    <div class="hint" v-if="focusActive.active && focusActiveBlocklist">
                      «{{ focusActiveBlocklist.name }}» —
                      {{ focusActiveBlocklist.domains.length }} доменов
                    </div>
                    <div class="hint" v-else-if="focusActive.active">
                      Включена (блок-лист id: {{ focusActive.blocklist_id }})
                    </div>
                    <div class="hint" v-else>Сейчас блокировка не активна</div>
                  </div>
                  <div class="row-actions">
                    <button
                      type="button"
                      class="btn ghost danger"
                      :disabled="!focusActive.active || focusBusy === '__deactivate__'"
                      @click="onDeactivate"
                    >
                      {{ focusBusy === "__deactivate__" ? "Отключение…" : "Отключить" }}
                    </button>
                  </div>
                </div>

                <!-- Список блок-листов как grid карточек -->
                <div class="ext-section-title">Блок-листы</div>

                <div v-if="focusLoading" class="empty">Загрузка…</div>

                <div v-else class="focus-grid">
                  <BlocklistCard
                    v-for="bl in focusBlocklists"
                    :key="bl.id"
                    :name="bl.name"
                    :domains="bl.domains"
                    :icon="bl.icon ?? ''"
                    :preset="bl.preset ?? false"
                    :active="focusActive.active && focusActive.blocklist_id === bl.id"
                    :count="bl.domains.length"
                    @click="openEditBlocklist(bl)"
                    @delete="onDeleteBlocklist(bl.id)"
                  />
                  <button
                    type="button"
                    class="add-card"
                    :disabled="focusEditing || focusBackendMissing"
                    @click="openCreateBlocklist"
                  >
                    + Создать блок-лист
                  </button>
                </div>

                <!-- Edit modal/inline form -->
                <div v-if="focusEditing" class="focus-create-form">
                  <div class="focus-create-row">
                    <label class="label" for="focus-blocklist-name">Название</label>
                    <input
                      id="focus-blocklist-name"
                      v-model="focusDraftName"
                      type="text"
                      class="focus-input"
                      placeholder="Например: Соцсети"
                      autocomplete="off"
                    />
                  </div>

                  <div class="focus-create-row">
                    <label class="label">Иконка</label>
                    <div class="icon-picker">
                      <button
                        v-for="ic in ICON_CHOICES"
                        :key="ic"
                        type="button"
                        class="icon-choice"
                        :class="{ selected: focusDraftIcon === ic }"
                        @click="focusDraftIcon = focusDraftIcon === ic ? '' : ic"
                      >
                        {{ ic }}
                      </button>
                    </div>
                  </div>

                  <div class="focus-create-row mention-host">
                    <label class="label" for="focus-blocklist-domains">
                      Домены (по одному на строку, # — комментарии, @ — ссылка на другой блок-лист)
                    </label>
                    <textarea
                      id="focus-blocklist-domains"
                      ref="mentionTextareaRef"
                      v-model="focusDraftDomains"
                      class="focus-textarea"
                      :class="{ 'drag-over': focusDragOver }"
                      spellcheck="false"
                      rows="8"
                      placeholder="# перетащи .txt сюда или вставь список&#10;twitter.com&#10;@preset:distractions"
                      @input="onDomainsInput"
                      @keydown="onMentionKey"
                      @dragover="onDraftDragOver"
                      @dragleave="onDraftDragLeave"
                      @drop="onDraftDrop"
                    />
                    <div
                      v-if="mentionOpen && mentionCandidates.length > 0"
                      class="mention-dropdown"
                    >
                      <button
                        v-for="(bl, i) in mentionCandidates"
                        :key="bl.id"
                        type="button"
                        class="mention-item"
                        :class="{ active: i === mentionHighlight }"
                        @mousedown.prevent="insertMention(bl)"
                      >
                        <span class="mention-icon">{{ bl.icon || "📁" }}</span>
                        <span class="mention-name">{{ bl.name }}</span>
                        <code class="mention-id">@{{ bl.id }}</code>
                      </button>
                    </div>
                    <div class="hint focus-parse-stats">
                      Записей: {{ focusDraftParsed.domains.length }}
                      <span v-if="focusDraftParsed.kind === 'raw'" class="ext-author">
                        · содержит ссылки
                      </span>
                      <span v-if="focusDraftParsed.invalid.length > 0" class="focus-invalid-count">
                        · невалидных: {{ focusDraftParsed.invalid.length }}
                      </span>
                    </div>
                  </div>
                  <div v-if="focusDraftError" class="error">{{ focusDraftError }}</div>
                  <div class="focus-create-actions">
                    <button
                      type="button"
                      class="btn ghost"
                      :disabled="focusBusy === '__create__'"
                      @click="cancelCreateBlocklist"
                    >
                      Отмена
                    </button>
                    <button
                      v-if="focusEditingId && !focusActive.active"
                      type="button"
                      class="btn"
                      :disabled="focusBusy === '__create__' || focusBackendMissing"
                      @click="onActivate(focusEditingId)"
                    >
                      Включить
                    </button>
                    <button
                      type="button"
                      class="btn"
                      :disabled="focusBusy === '__create__' || focusBackendMissing"
                      @click="onCreateBlocklist"
                    >
                      {{ focusBusy === "__create__" ? "Сохранение…" : "Сохранить" }}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </section>
        </template>

        <!-- Export tab — список зарегистрированных converters + history -->
        <template v-else-if="activeTab === 'export'">
          <div v-if="exportError" class="error-banner">{{ exportError }}</div>

          <div v-if="exportLoading" class="empty">Загрузка…</div>

          <div v-else class="rows kosmos-scroll">
            <div v-if="exportConverters.length === 0" class="empty">
              Нет доступных конвертеров. Backend ещё не зарегистрировал ни одного.
            </div>

            <div v-for="c in exportConverters" :key="c.converter_id" class="row export-row">
              <div class="row-label">
                <div class="label">{{ c.display_name }}</div>
                <div class="hint">
                  {{ c.object_type }} →
                  {{ exportSelectedFormat[c.converter_id] ?? c.default_format }}
                </div>
                <div v-if="exportStatusByConverter[c.converter_id]" class="hint export-status">
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
                  <option v-for="f in c.supported_formats" :key="f" :value="f">
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

            <div v-if="exportHistory.length > 0" class="ext-section-header">Последние экспорты</div>
            <div v-for="(h, i) in exportHistory" :key="i" class="row export-history-row">
              <div class="row-label">
                <div class="label">
                  {{ h.display_name }}
                  <span class="hint">({{ h.format }})</span>
                </div>
                <div class="hint">
                  {{ formatHistoryTime(h.timestamp) }} · {{ h.file_count }} файлов ·
                  {{ (h.bytes / 1024).toFixed(1) }} KB
                </div>
                <code class="value lock">{{ h.dest_dir }}</code>
              </div>
              <div class="row-actions">
                <span class="value" :class="{ muted: !h.ok }">
                  {{ h.ok ? "ok" : "с ошибками" }}
                </span>
              </div>
            </div>
          </div>
        </template>
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
  background: var(--main-background-color);
  outline: none;
}

.settings-shell {
  display: flex;
  min-height: 0;
  flex: 1;
}

.settings-content {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  background: var(--main-background-color);
}

.settings-sidebar-group {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.settings-sidebar-empty {
  padding: 4px 6px;
  color: var(--second-text-color);
  font-size: 13px;
  line-height: 1.4;
  font-weight: 500;
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
  to {
    transform: rotate(360deg);
  }
}

.row-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.content-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 36px;
  padding: 8px 140px 4px 10px;
  -webkit-app-region: drag;
  gap: 16px;
}

.content-header__nav,
.content-header__right {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  -webkit-app-region: no-drag;
}

.chrome-control {
  display: inline-flex;
  width: 28px;
  height: 28px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: none;
  border-radius: 7px;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 72%, transparent);
  cursor: pointer;
  -webkit-app-region: no-drag;
}

.chrome-control:hover:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.chrome-control:disabled {
  cursor: default;
  opacity: 0.32;
}

.chrome-control--danger:hover:not(:disabled) {
  background: #c42b1c;
  color: #fff;
}

.advanced-feature-toggle {
  position: relative;
  width: 34px;
  height: 20px;
  margin-right: 6px;
  padding: 0;
  border: none;
  border-radius: 999px;
  background: transparent;
  cursor: default;
  opacity: 0.55;
  -webkit-app-region: no-drag;
}

.advanced-feature-toggle::before {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: color-mix(in srgb, var(--foreground) 18%, transparent);
}

.advanced-feature-toggle__thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--foreground) 62%, #000 38%);
  box-shadow: 0 1px 3px color-mix(in srgb, #000 24%, transparent);
}

.advanced-page {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 60px 16px 16px;
}

.advanced-page__body {
  margin-top: 60px;
  gap: 30px;
  display: flex;
  flex-direction: column;
}

.advanced-section-title {
  margin: 0 0 10px;
  color: var(--foreground);
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 1.4;
  font-weight: 500;
}

.advanced-page .rows {
  flex: initial;
  overflow: visible;
  padding: 0;
}

.advanced-page .ext-list {
  flex: initial;
  overflow: visible;
  padding: 0;
}

.advanced-page .ext-footer {
  padding: 12px 0 0;
}

.rows {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 0;
}

.row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  padding: 14px 12px;
  background: var(--settings-list-background);
  border-bottom: 1px solid var(--border-color-strong);
}

.rows > .row:first-child {
  border-top-left-radius: 12px;
  border-top-right-radius: 12px;
}

.row:last-child {
  border-bottom: none;
}

.rows > .row:last-child {
  border-bottom-right-radius: 12px;
  border-bottom-left-radius: 12px;
}

.row-label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

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

.label {
  color: var(--foreground);
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 1.4;
  font-weight: 500;
}

.hint {
  color: var(--second-text-color);
  font-family: var(--font-sans);
  font-size: 11px;
  line-height: 1.4;
  font-weight: 500;
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

.ttl-input {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
  color: var(--kosmos-muted-fg);
  font-size: 13px;
}
.ttl-input input {
  width: 64px;
  padding: 6px 8px;
  font-family: inherit;
  font-size: 13px;
  color: var(--kosmos-fg);
  background: var(--vp-c-bg-soft, var(--kosmos-sidebar-surface));
  border: 1px solid var(--kosmos-border);
  border-radius: 6px;
  text-align: right;
}
.ttl-input input:focus {
  outline: none;
  border-color: var(--vp-c-brand-1, var(--kosmos-fg));
}

.toggle {
  position: relative;
  display: inline-block;
  cursor: pointer;
  flex-shrink: 0;
}

.toggle.disabled {
  cursor: not-allowed;
  opacity: 0.5;
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
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ext-section-title {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--muted-foreground);
  padding: 8px 4px 4px;
  margin-top: 4px;
}
.ext-section-title:first-child {
  margin-top: 0;
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
  /* gap 4px между токенами (версия / dev badge / автор / backup count) —
     раньше токены липли друг к другу, выглядело как один слово `v0.1.6·Kazui`. */
  display: flex;
  flex-wrap: wrap;
  column-gap: 4px;
  align-items: baseline;
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

/* Focus tab */

.focus-active-row {
  background: var(--settings-list-background);
}

.focus-service-row {
  background: var(--settings-list-background);
}

.focus-service-hint-ok {
  color: color-mix(in srgb, #4ade80 65%, var(--foreground) 35%);
}

.focus-item {
  align-items: flex-start;
}

.focus-domains-preview {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 11px;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  word-break: break-all;
}

.focus-create-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px;
  margin-top: 6px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  border: 1px solid color-mix(in srgb, var(--foreground) 10%, transparent);
}

.focus-create-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.focus-input {
  font: inherit;
  font-size: 12px;
  padding: 6px 10px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: color-mix(in srgb, oklch(0.04 0 0) 60%, transparent);
  color: var(--foreground);
  outline: none;
}

.focus-input:focus {
  border-color: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 60%, transparent);
}

.file-search-row {
  align-items: flex-start;
}

.file-search-wide {
  width: 100%;
}

.file-search-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 8px;
  list-style: none;
  padding: 0;
}

.file-search-chip {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-width: 0;
  padding: 9px 12px;
  border-radius: 10px;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  border: 1px solid color-mix(in srgb, var(--foreground) 7%, transparent);
  transition:
    background 120ms ease,
    border-color 120ms ease;
}

.file-search-chip:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  border-color: color-mix(in srgb, var(--foreground) 12%, transparent);
}

.file-search-chip__icon {
  flex: 0 0 auto;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  display: flex;
  align-items: center;
}

.file-search-chip code {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: color-mix(in srgb, var(--foreground) 85%, transparent);
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  direction: ltr;
}

/* Regression 2026-05-24-evening: delete button hidden until hover/focus. */
.file-search-chip .btn.ghost.danger {
  opacity: 0;
  transition: opacity 120ms ease;
  flex: 0 0 auto;
}

.file-search-chip:hover .btn.ghost.danger,
.file-search-chip:focus-within .btn.ghost.danger {
  opacity: 1;
}

.file-search-add {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.file-search-add .focus-input {
  min-width: 180px;
}

.focus-textarea {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  padding: 8px 10px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: color-mix(in srgb, oklch(0.04 0 0) 60%, transparent);
  color: var(--foreground);
  outline: none;
  resize: vertical;
  min-height: 120px;
  line-height: 1.5;
}

.focus-textarea:focus {
  border-color: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 60%, transparent);
}

.focus-textarea.drag-over {
  border-color: var(--accent, oklch(0.7 0.18 250));
  background: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 12%, transparent);
}

.focus-parse-stats {
  margin-top: 2px;
}

.focus-invalid-count {
  color: oklch(0.65 0.22 25);
}

.focus-create-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

/* Focus grid (карточки BlocklistCard) */

.focus-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
  padding: 12px 4px;
}

.add-card {
  min-height: 140px;
  border: 2px dashed color-mix(in srgb, var(--foreground) 16%, transparent);
  border-radius: 12px;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  cursor: pointer;
  transition:
    border-color 120ms ease,
    color 120ms ease;
  font: inherit;
  font-size: 13px;
  font-weight: 500;
}

.add-card:hover:not(:disabled) {
  border-color: var(--accent, oklch(0.7 0.18 250));
  color: var(--foreground);
}

.add-card:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Icon picker */

.icon-picker {
  display: grid;
  grid-template-columns: repeat(10, 1fr);
  gap: 4px;
  max-width: 360px;
}

.icon-choice {
  font: inherit;
  font-size: 16px;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
  cursor: pointer;
}

.icon-choice:hover {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
}

.icon-choice.selected {
  border-color: var(--accent, oklch(0.7 0.18 250));
  background: color-mix(in srgb, var(--accent, oklch(0.7 0.18 250)) 18%, transparent);
}

/* Mention autocomplete dropdown */

.mention-host {
  position: relative;
}

.mention-dropdown {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 32px;
  z-index: 20;
  background: color-mix(in srgb, oklch(0.04 0 0) 96%, transparent);
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 220px;
  overflow-y: auto;
}

.mention-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: none;
  background: transparent;
  color: var(--foreground);
  border-radius: 6px;
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  text-align: left;
}

.mention-item:hover,
.mention-item.active {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
}

.mention-icon {
  font-size: 14px;
}

.mention-name {
  flex: 1;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mention-id {
  font-size: 10px;
  font-family: var(--font-mono, ui-monospace, monospace);
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
}

/* Dictation + Security pages — leverage @kosmos/visuals primitives. */
.security-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px;
  overflow-y: auto;
}

.security-page__intro {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--muted-foreground);
}

.advanced-page__body :deep(.kosmos-settings-list) {
  margin-bottom: 12px;
}

.control-stack {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.control-hint {
  font-size: 11px;
  color: var(--muted-foreground);
}

/* Dictation stats — three-card row, Raycast-style. */
.stats-header {
  display: flex;
  align-items: center;
  margin: 0 2px 0;
  font-size: 11px;
  color: var(--muted-foreground);
}

.stats-header__title {
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.stats-cards {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 1px;
  margin-bottom: 16px;
  background: var(--border);
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.7);
  corner-shape: var(--corner-shape);
  overflow: hidden;
}

.stats-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 14px 16px;
  background: var(--settings-list-background, var(--background));
}

.stats-card__label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--muted-foreground);
  font-weight: 500;
}

.stats-card__value {
  display: flex;
  align-items: baseline;
  gap: 4px;
}

.stats-card__number {
  font-size: 28px;
  font-weight: 600;
  color: var(--foreground);
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
  line-height: 1;
}

.stats-card__unit {
  font-size: 12px;
  font-weight: 500;
  color: var(--muted-foreground);
}
</style>
