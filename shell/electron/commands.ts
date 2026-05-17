// Static command registry — open-commands ("Открыть <App>") открывают
// апки как Vue extension'ы внутри Kepler через extension-host (Phase 4 + 6).
//
// Action-commands (Pomodoro start, create note и т.п.) приходят dynamic от
// running extension'ов через kepler-backend command bus.

import { extensionIconDataUri, openExtension } from "./extension-host";
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
   * Опциональная иконка как data URI. Для open-команд extension'ов берётся
   * из `extensionIconDataUri(<id>)`. Lazy getter — читаем с диска один раз,
   * результат кешируется в extension-host.
   */
  icon?: () => string | undefined;
  exec: () => Promise<void> | void;
}

function openAsExtension(id: string, route?: string): void {
  openExtension(id, route);
}

async function runCheckUpdates(): Promise<void> {
  try {
    await checkUpdates();
  } catch (e) {
    console.warn("[kepler-shell] check updates failed:", e);
  }
}

export const COMMANDS: InternalCommand[] = [
  // App tiles — открывают приложение/extension в новом окне.
  {
    id: "delphi:open",
    title: "Открыть Delphi",
    subtitle: "Задачи",
    category: "open",
    kind: "app",
    icon: () => extensionIconDataUri("delphi"),
    exec: () => openAsExtension("delphi", "/today"),
  },
  {
    id: "horologion:open",
    title: "Открыть Horologion",
    subtitle: "Pomodoro + трекер времени",
    category: "open",
    kind: "app",
    icon: () => extensionIconDataUri("horologion"),
    exec: () => openAsExtension("horologion"),
  },
  {
    id: "arrancador:open",
    title: "Открыть Arrancador",
    subtitle: "Игровая библиотека",
    category: "open",
    kind: "app",
    icon: () => extensionIconDataUri("arrancador"),
    exec: () => openAsExtension("arrancador"),
  },
  {
    id: "eden:open",
    title: "Открыть Eden",
    subtitle: "Заметки и дневник",
    category: "open",
    kind: "app",
    icon: () => extensionIconDataUri("eden"),
    exec: () => openAsExtension("eden"),
  },
  // Kepler commands — встроенные в shell.
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
  // Extension commands — открывают приложение на конкретной странице.
  {
    id: "delphi:inbox",
    title: "Открыть входящие",
    subtitle: "Delphi",
    category: "open",
    kind: "command",
    appName: "Delphi",
    icon: () => extensionIconDataUri("delphi"),
    exec: () => openAsExtension("delphi", "/"),
  },
  {
    id: "horologion:pomodoro",
    title: "Помодоро",
    subtitle: "Horologion",
    category: "open",
    kind: "command",
    appName: "Horologion",
    icon: () => extensionIconDataUri("horologion"),
    exec: () => openAsExtension("horologion", "/?mode=pomodoro"),
  },
  {
    id: "horologion:stopwatch",
    title: "Секундомер",
    subtitle: "Horologion",
    category: "open",
    kind: "command",
    appName: "Horologion",
    icon: () => extensionIconDataUri("horologion"),
    exec: () => openAsExtension("horologion", "/?mode=stopwatch"),
  },
];

export function findCommand(id: string): InternalCommand | undefined {
  return COMMANDS.find((c) => c.id === id);
}
