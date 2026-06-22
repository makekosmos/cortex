// Static configuration: settings tabs + per-tab app-commands.
//
// Вынесено из `SettingsView.vue` (1-line вывоз через `import { ... }
// from "./settings/navigation"` для 280-string данных, которые раньше
// зашоривали script section). Только данные — без reactive state.

import type { Component } from "vue";
import {
  BookOpen,
  Bug,
  Clipboard,
  Gamepad2,
  Info,
  ListTodo,
  Mic,
  MicVocal,
  Puzzle,
  Search,
  Settings,
  Shield,
  ShieldCheck,
  Timer,
  RefreshCw,
} from "@lucide/vue";
import delphiSvg from "../../assets/delphi.svg";
import delphiAddSvg from "../../assets/delphi-add.svg";
import arraSvg from "../../assets/arra.svg";
import edenSvg from "../../assets/eden.svg";
import edenAddSvg from "../../assets/eden-add.svg";
import edenDiarySvg from "../../assets/eden-diary.svg";
import kosmosIconPng from "../../../build/icon.png";
import { CLIPBOARD_HISTORY_ENABLED } from "@shared/ipc-types";

export type Tab =
  | "general"
  | "about"
  | "debug"
  | "security"
  | "sync"
  | "secrets"
  | "notes"
  | "tasks"
  | "time-tracker"
  | "games"
  | "extensions"
  | "focus"
  | "clipboard"
  | "ai"
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

export type AppSettingsTab = "notes" | "tasks" | "time-tracker" | "games" | "dictation";

export interface AppCommandSetting {
  id: string;
  title: string;
  icon: string;
  iconFrom: string;
  iconTo: string;
  shortcut?: string;
}

export const HIDDEN_COMMANDS_KEY = "kepler.launcher.hiddenCommandIds";

const EDEN_COMMAND_GRADIENT = { iconFrom: "#ff5c00", iconTo: "#b33800" };
const DELPHI_COMMAND_GRADIENT = {
  iconFrom: "oklch(0.78 0.14 230)",
  iconTo: "oklch(0.5 0.18 245)",
};
const FOCUS_COMMAND_GRADIENT = {
  iconFrom: "oklch(0.7 0.16 145)",
  iconTo: "oklch(0.46 0.14 165)",
};
const ARRANCADOR_COMMAND_GRADIENT = {
  iconFrom: "oklch(0.7 0.2 25)",
  iconTo: "oklch(0.45 0.18 20)",
};
const DICTATION_COMMAND_GRADIENT = {
  iconFrom: "#F472B6",
  iconTo: "#BE185D",
};

export const appCommandSettings: Record<AppSettingsTab, AppCommandSetting[]> = {
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
      id: "kepler:focus-session",
      title: "Начать фокус",
      icon: kosmosIconPng,
      ...FOCUS_COMMAND_GRADIENT,
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
  dictation: [
    {
      id: "kepler:dictation",
      title: "Переключить диктовку",
      icon: kosmosIconPng,
      ...DICTATION_COMMAND_GRADIENT,
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
    tab: "sync",
    label: "Синхронизация",
    group: "main",
    layout: "basic",
    icon: RefreshCw,
    description: "Устройства, код подключения и состояние синхронизации.",
    keywords: [
      "синхронизация",
      "sync",
      "устройства",
      "ноды",
      "nodes",
      "код",
      "подключение",
      "pairing",
      "iroh",
    ],
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
    tab: "ai",
    label: "AI",
    group: "advanced",
    layout: "advanced",
    icon: MicVocal,
    iconGradient: { from: "#F472B6", to: "#BE185D" },
    description: "Модели и API-ключи для AI.",
    keywords: [
      "ai",
      "искусственный интеллект",
      "модель",
      "модели",
      "локальная модель",
      "локально",
      "groq",
      "whisper",
      "speech to text",
      "stt",
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
      "focus",
    ],
  },
  {
    tab: "dictation",
    label: "Диктация",
    group: "advanced",
    layout: "advanced",
    icon: Mic,
    iconGradient: { from: "#F472B6", to: "#7C2D12" },
    description: "Голосовой ввод локально или через подключённый провайдер.",
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
  // Буфер обмена заморожен — вкладка скрыта (CLIPBOARD_HISTORY_ENABLED).
  ...(CLIPBOARD_HISTORY_ENABLED
    ? [
        {
          tab: "clipboard",
          label: "Буфер обмена",
          group: "advanced",
          layout: "advanced",
          icon: Clipboard,
          iconGradient: {
            from: "var(--accent)",
            to: "color-mix(in srgb, var(--accent) 58%, var(--background))",
          },
          description: "История скопированных данных, срок хранения и лимит места.",
          keywords: [
            "буфер обмена",
            "clipboard",
            "история буфера",
            "копирование",
            "retention",
            "пины",
            "срок хранения",
            "лимит места",
          ],
        } satisfies SettingsNavigationItem,
      ]
    : []),
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
    label: "Фокус-таймер",
    group: "advanced",
    layout: "advanced",
    icon: Timer,
    iconGradient: { from: "#F59E0B", to: "#92400E" },
    sidebarImage: kosmosIconPng,
    introImage: kosmosIconPng,
    description: "Shell-owned фокус-сессии, задача и таймер.",
    keywords: [
      "фокус-таймер",
      "трекер времени",
      "time tracker",
      "pomodoro",
      "таймер",
      "focus",
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
