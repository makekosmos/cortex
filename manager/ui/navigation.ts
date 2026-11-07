import type { NavGroup } from "@makekosmos/imago-nsh";

/**
 * Sidebar groups of the Manager. The first group answers "is my Mundus well",
 * the second covers what runs inside it, the third the machines and their data,
 * and the last two the app's own configuration.
 */
export const navigation: NavGroup[] = [
  // The server on its own: whether Mundus is well is a different question
  // from what runs inside it, so it sits above the first group.
  {
    id: "server",
    entries: [
      {
        route: "server",
        label: "Сервер",
        icon: "server",
        accent: "#3b82f6",
        accentTo: "#359ecb",
        caption: "Работает",
      },
    ],
  },
  {
    id: "app",
    entries: [
      {
        route: "about",
        label: "О приложении",
        icon: "info",
        accent: "#1784fc",
        accentTo: "#915bfc",
      },
      {
        route: "appearance",
        label: "Внешний вид",
        icon: "palette",
        accent: "#f79411",
        accentTo: "spectrum",
      },
    ],
  },
  {
    id: "running",
    entries: [
      { route: "apps", label: "Приложения", icon: "apps", accent: "#fa6710", accentTo: "#fc2d3e" },
      {
        route: "integrations",
        label: "Интеграции",
        icon: "plug",
        accent: "#915bfc",
        accentTo: "#fb3499",
      },
      {
        route: "storage",
        label: "Хранилище",
        icon: "database",
        accent: "#15b8a4",
        accentTo: "#1784fc",
      },
      {
        route: "activity",
        label: "Активность",
        icon: "pulse",
        accent: "#fa6710",
        accentTo: "#fc2d3e",
      },
    ],
  },
  {
    id: "machines",
    entries: [
      {
        route: "devices",
        label: "Девайсы",
        icon: "monitor",
        accent: "#1784fc",
        accentTo: "#15b8a4",
      },
      { route: "browser", label: "Браузер", icon: "globe", accent: "#915bfc", accentTo: "#1784fc" },
    ],
  },
  {
    id: "data",
    entries: [
      { route: "keys", label: "Ключи", icon: "key", accent: "#f79411", accentTo: "#fa6710" },
      { route: "models", label: "Модели", icon: "chip", accent: "#fb3499", accentTo: "#915bfc" },
    ],
  },
  {
    id: "config",
    entries: [
      {
        route: "settings",
        label: "Настройки",
        icon: "gear",
        accent: "#777777",
        accentTo: "#777777",
      },
      {
        route: "developer",
        label: "Разработчикам",
        icon: "code",
        accent: "#777777",
        accentTo: "#777777",
      },
    ],
  },
];
