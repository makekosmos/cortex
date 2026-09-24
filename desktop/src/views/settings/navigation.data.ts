import { Bug, FolderSearch, Info, Settings } from "@lucide/vue";
import kosmosIconPng from "../../../build/icon.png";

import type { SettingsNavigationItem } from "./navigation";

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
    description: "Kosmos и обновления приложения.",
    keywords: ["about", "о приложении", "версия", "kepler", "kosmos", "обновления", "update"],
  },
  {
    tab: "file-index",
    label: "Индекс файлов",
    group: "advanced",
    layout: "advanced",
    icon: FolderSearch,
    description: "Shell-owned поиск файлов через локальный Engine.",
    keywords: ["индекс файлов", "поиск файлов", "папки", "сканирование", "file index"],
  },
];
