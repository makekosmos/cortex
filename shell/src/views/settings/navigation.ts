// Static configuration: settings tabs + per-tab app-commands.
//
// Вынесено из `SettingsView.vue` (1-line вывоз через `import { ... }
// from "./settings/navigation"` для 280-string данных, которые раньше
// зашоривали script section). Только данные — без reactive state.

import type { Component } from "vue";
import {
  BookOpen,
  Bug,
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
import holoSvg from "../../assets/holo.svg";
import holoPomoSvg from "../../assets/holo-pomo.svg";
import holoSecoSvg from "../../assets/holo-seco.svg";
import horoLogoPng from "../../assets/horo-logo.png";
import delphiSvg from "../../assets/delphi.svg";
import delphiAddSvg from "../../assets/delphi-add.svg";
import arraSvg from "../../assets/arra.svg";
import edenSvg from "../../assets/eden.svg";
import edenAddSvg from "../../assets/eden-add.svg";
import edenDiarySvg from "../../assets/eden-diary.svg";
import kosmosIconPng from "../../../build/icon.png";

export type Tab =
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

export interface SettingsNavigationItem {
  tab: Tab;
  label: string;
  group: "main" | "advanced";
  layout: "basic" | "advanced";
  icon: Component;
  iconGradient?: { from: string; to: string };
  sidebarImage?: string;
  introImage?: string;
  description?: string;
  keywords: string[];
}

export type AppSettingsTab = "notes" | "tasks" | "time-tracker" | "games";

export interface AppCommandSetting {
  id: string;
  title: string;
  icon: string;
  iconFrom: string;
  iconTo: string;
}

export const HIDDEN_COMMANDS_KEY = "kepler.launcher.hiddenCommandIds";

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

export const appCommandSettings: Record<AppSettingsTab, AppCommandSetting[]> = {
  notes: [
    { id: "eden:open", title: "Открыть Eden", icon: edenSvg, ...EDEN_COMMAND_GRADIENT },
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
    { id: "delphi:open", title: "Открыть Delphi", icon: delphiSvg, ...DELPHI_COMMAND_GRADIENT },
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

export const settingsNavigationItems: SettingsNavigationItem[] = [
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
