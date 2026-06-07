// Static command registry — open-commands ("Открыть <App>") открывают
// апки как Vue extension'ы внутри Kepler через extension-host (Phase 4 + 6).
//
// Action-commands (Pomodoro start, create note и т.п.) приходят dynamic от
// running extension'ов через kepler-backend command bus.

// extensionIconDataUri / openExtension больше не используются здесь —
// extension-команды объявляются в их manifest.commands[] и резолвятся
// через `loadDeclaredCommands` (extension-host.ts).
import { openDashboardWindow } from "./dashboard-window";
import { openClipboardHistoryShell } from "./clipboard-history";
import { CLIPBOARD_HISTORY_ENABLED } from "../shared/ipc-types";
import {
  openFocusSessionShell,
  pauseFocusSessionCommand,
  resumeFocusSessionCommand,
  skipFocusSessionCommand,
  stopFocusSessionCommand,
  toggleFocusSessionCommand,
} from "./focus-session";
import { openSettings } from "./settings-window";
import { check as checkUpdates } from "./autoupdater-host";
import type { IpcMainInvokeEvent } from "electron";

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
  keepsLauncherOpen?: boolean;
  exec: (event?: IpcMainInvokeEvent) => Promise<void> | void;
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
  // Буфер обмена заморожен — команда скрыта (CLIPBOARD_HISTORY_ENABLED).
  ...(CLIPBOARD_HISTORY_ENABLED
    ? [
        {
          id: "kepler:clipboard-history",
          title: "Открыть буфер обмена",
          subtitle: "История скопированного текста",
          category: "open",
          kind: "command",
          appName: "Kepler",
          keepsLauncherOpen: true,
          exec: () => openClipboardHistoryShell(),
        } satisfies InternalCommand,
      ]
    : []),
  {
    id: "kepler:focus-session",
    title: "Начать фокус",
    subtitle: "Таймер, задача и блокировка отвлечений",
    category: "open",
    kind: "command",
    appName: "Kosmos",
    keepsLauncherOpen: true,
    exec: () => openFocusSessionShell(),
  },
  {
    id: "kepler:focus-toggle",
    title: "Переключить фокус",
    subtitle: "Начать новую сессию или завершить текущую",
    category: "action",
    kind: "command",
    appName: "Kosmos",
    exec: () => toggleFocusSessionCommand(),
  },
  {
    id: "kepler:focus-pause",
    title: "Поставить фокус на паузу",
    subtitle: "Временно остановить текущую фокус-сессию",
    category: "action",
    kind: "command",
    appName: "Kosmos",
    exec: async () => {
      await pauseFocusSessionCommand();
    },
  },
  {
    id: "kepler:focus-resume",
    title: "Продолжить фокус",
    subtitle: "Вернуться к текущей фокус-сессии",
    category: "action",
    kind: "command",
    appName: "Kosmos",
    exec: async () => {
      await resumeFocusSessionCommand();
    },
  },
  {
    id: "kepler:focus-skip",
    title: "Пропустить фазу фокуса",
    subtitle: "Перейти к следующей фазе pomodoro",
    category: "action",
    kind: "command",
    appName: "Kosmos",
    exec: async () => {
      await skipFocusSessionCommand();
    },
  },
  {
    id: "kepler:focus-complete",
    title: "Завершить фокус",
    subtitle: "Остановить текущую фокус-сессию",
    category: "action",
    kind: "command",
    appName: "Kosmos",
    exec: async () => {
      await stopFocusSessionCommand();
    },
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
