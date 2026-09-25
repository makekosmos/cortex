// Static command registry — open-commands ("Открыть <App>") открывают
//
// Action-commands (Pomodoro start, create note и т.п.) приходят dynamic от
// running extension'ов через kepler-backend command bus.

import { openAgenda } from "./agenda-navigation";
import { openHostedApp } from "./host-app";
import { toggleDictation } from "./dictation-pill";
import { DEFAULT_HOTKEY, openSettings } from "./settings-window";
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
   * internal команды (settings/check-updates) оставляют поле
   * undefined и видны всегда.
   */
  /**
   * Опциональная иконка как data URI. Для open-команд extension'ов берётся
   */
  icon?: () => string | undefined;
  shortcut?: string | (() => string | undefined | Promise<string | undefined>);
  exec: (event?: IpcMainInvokeEvent) => Promise<void> | void;
}

type DictationShortcutResolver = () => string | undefined | Promise<string | undefined>;
let dictationShortcutResolver: DictationShortcutResolver | null = null;

export function setDictationShortcutResolver(resolver: DictationShortcutResolver | null): void {
  dictationShortcutResolver = resolver;
}

async function resolveDictationShortcut(): Promise<string> {
  try {
    const live = await dictationShortcutResolver?.();
    return isNonEmptyString(live) ? live.trim() : DEFAULT_HOTKEY;
  } catch {
    return DEFAULT_HOTKEY;
  }
}

function isNonEmptyString(value: string | undefined): value is string {
  return typeof value === "string" && value.trim().length > 0;
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
  {
    id: "kosmos:my-cosmos",
    title: "Мой космос",
    subtitle: "Граф объектов ARK",
    category: "open",
    kind: "command",
    appName: "Kosmos",
    exec: () => openHostedApp("com.kosmos.graph"),
  },
  {
    // KOS-137: GPUI Agenda ships inside the installer as
    // components/agenda/Kosmos Agenda.exe — same Engine lock as Manager.
    // The Vue Agenda (com.kosmos.agenda package) stays installed as fallback.
    id: "kosmos:agenda-gpui",
    title: "Открыть Agenda (GPUI)",
    subtitle: "Задачи · нативная оболочка",
    category: "open",
    kind: "command",
    appName: "Kosmos",
    exec: () => openAgenda(),
  },
  {
    id: "kepler:dictation",
    title: "Переключить диктовку",
    subtitle: "Начать или остановить голосовой ввод",
    category: "action",
    kind: "command",
    appName: "Kosmos",
    shortcut: () => resolveDictationShortcut(),
    exec: () => toggleDictation(),
  },
  {
    id: "settings:open",
    title: "Открыть настройки",
    subtitle: "Kosmos",
    category: "open",
    kind: "command",
    appName: "Kosmos",
    exec: () => openSettings(),
  },
  {
    id: "kepler:check-updates",
    title: "Проверить обновления",
    subtitle: "Kosmos и расширения",
    category: "open",
    kind: "command",
    appName: "Kosmos",
    exec: () => runCheckUpdates(),
  },
];

export function findCommand(id: string): InternalCommand | undefined {
  return COMMANDS.find((c) => c.id === id);
}
