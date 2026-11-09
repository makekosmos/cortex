import type { IconName } from "@makekosmos/imago-nsh";

/**
 * One row of the Manager's sidebar: a page, its name and a plain icon.
 *
 * There is no accent colour to give: the rows are monochrome on purpose, so the
 * only thing that stands out is the one row that is the product itself
 * (`head`), which wears its own mark.
 */
export type NavRow = {
  route: string;
  label: string;
  icon: IconName;
  /** The product's own row: a mark on a dark tile, not a page's icon. */
  head?: boolean;
};

export type NavSection = { id: string; rows: NavRow[] };

/**
 * What the Manager has, in the order the sidebar shows it: the product itself,
 * then what runs inside it, then the machines, then the person's own additions
 * (a plugin's page), and last how it looks and behaves.
 */
export const navigation: NavSection[] = [
  {
    id: "server",
    rows: [{ route: "server", label: "Mundus", icon: "mundus", head: true }],
  },
  {
    id: "running",
    rows: [
      { route: "plugins", label: "Плагины", icon: "apps" },
      { route: "integrations", label: "Интеграции", icon: "plug" },
      { route: "storage", label: "Хранилище", icon: "database" },
      { route: "activity", label: "Активность", icon: "pulse" },
    ],
  },
  {
    id: "machines",
    rows: [
      { route: "devices", label: "Девайсы", icon: "monitor" },
      { route: "browser", label: "Браузер", icon: "globe" },
    ],
  },
  {
    id: "data",
    rows: [
      { route: "keys", label: "Ключи", icon: "key" },
      { route: "models", label: "Модели", icon: "chip" },
    ],
  },
];

/** What sits under the person's additions: the app's own configuration. */
export const configuration: NavSection = {
  id: "config",
  rows: [
    { route: "appearance", label: "Внешний вид", icon: "palette" },
    { route: "settings", label: "Настройки", icon: "gear" },
  ],
};
