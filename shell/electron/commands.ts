// Static command registry — open-commands ("Открыть <App>") открывают
// апки как Vue extension'ы внутри Kepler через extension-host (Phase 4 + 6).
//
// Action-commands (Pomodoro start, create note и т.п.) приходят dynamic от
// running extension'ов через kepler-backend command bus.

// extensionIconDataUri / openExtension больше не используются здесь —
// extension-команды объявляются в их manifest.commands[] и резолвятся
// через `loadDeclaredCommands` (extension-host.ts).
import { openDashboardWindow } from "./dashboard-window";
import { openSettings } from "./settings-window";
import { check as checkUpdates } from "./autoupdater-host";

export interface InternalCommand {
  id: string;
  title: string;
  subtitle: string;
  category: "open" | "action";
  /** UI-классификация плашки. 'app' → правый лейбл «Приложение».
      'command' → «Команда · <appName>». */
  kind?: "app" | "command";
  /** Имя родительского приложения для command-плашек. */
  appName?: string;
  /**
   * Если задано — команда видна только когда extension с этим id установлен
   * (`%APPDATA%\Kosmos\extensions\<id>\manifest.json` существует). Kepler-
   * internal команды (settings/dashboard/check-updates) оставляют поле
   * undefined и видны всегда.
   */
  requiresExtension?: string;
  /**
   * Опциональная иконка как data URI. Для open-команд extension'ов берётся
   * из `extensionIconDataUri(<id>)`. Lazy getter — читаем с диска один раз,
   * результат кешируется в extension-host.
   */
  icon?: () => string | undefined;
  exec: () => Promise<void> | void;
}

async function runCheckUpdates(): Promise<void> {
  try {
    await checkUpdates();
  } catch (e) {
    console.warn("[kepler-shell] check updates failed:", e);
  }
}

export const COMMANDS: InternalCommand[] = [
  // Kepler-internal команды (shell-owned, не extensions). Extension'ы
  // объявляют свои команды в `manifest.commands[]` — см.
  // `loadDeclaredCommands` в extension-host.ts и docs-site/concepts/command-bus.md.
  {
    id: "dashboard:open",
    title: "Открыть таблицу данных",
    subtitle: "Просмотр объектов ARK",
    category: "open",
    kind: "command",
    appName: "Kepler",
    exec: () => openDashboardWindow(),
  },
  {
    id: "settings:open",
    title: "Открыть настройки",
    subtitle: "Kepler",
    category: "open",
    kind: "command",
    appName: "Kepler",
    exec: () => openSettings(),
  },
  {
    id: "kepler:check-updates",
    title: "Проверить обновления",
    subtitle: "Kepler и расширения",
    category: "open",
    kind: "command",
    appName: "Kepler",
    exec: () => runCheckUpdates(),
  },
];

export function findCommand(id: string): InternalCommand | undefined {
  return COMMANDS.find((c) => c.id === id);
}
