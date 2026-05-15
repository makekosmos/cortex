// Static command registry — open-commands ("Открыть <App>") теперь открывают
// апки как Vue extension'ы внутри Kepler через extension-host (Phase 4).
// Spawn .exe path остаётся fallback'ом для Eden (она ещё standalone Electron).
//
// Action-commands (Pomodoro start, create note и т.п.) приходят dynamic от
// running extension'ов через kepler-backend command bus.

import { extensionIconDataUri, openExtension } from "./extension-host";
import { openDashboardWindow } from "./dashboard-window";
import { openSettings } from "./settings-window";

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// ESM shim — __dirname / __filename не определены в Node ESM bundles.
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export interface InternalCommand {
  id: string;
  title: string;
  subtitle: string;
  category: "open" | "action";
  /**
   * Опциональная иконка как data URI. Для open-команд extension'ов берётся
   * из `extensionIconDataUri(<id>)`. Lazy getter — читаем с диска один раз,
   * результат кешируется в extension-host.
   */
  icon?: () => string | undefined;
  exec: () => Promise<void> | void;
}

function resolveAppExe(appLower: string): string | null {
  const cap = appLower.charAt(0).toUpperCase() + appLower.slice(1);
  const candidates = [
    path.join(process.env.LOCALAPPDATA ?? "", "Kosmos", cap, `${cap}.exe`),
    path.join(process.env.LOCALAPPDATA ?? "", "Programs", cap, `${cap}.exe`),
    path.join(process.env.PROGRAMFILES ?? "", cap, `${cap}.exe`),
    path.join(process.env.PROGRAMFILES ?? "", "Kosmos", cap, `${cap}.exe`),
    path.resolve(__dirname, "..", "..", "..", "..", "apps", appLower, "release", `${cap}.exe`),
    path.resolve(__dirname, "..", "..", "..", "..", "apps", appLower, "ts", "release", `${cap}.exe`),
  ];
  for (const c of candidates) {
    if (c && existsSync(c)) return c;
  }
  return null;
}

function openAppExe(appLower: string): void {
  const exe = resolveAppExe(appLower);
  if (!exe) {
    console.warn(`[kepler-shell] ${appLower}.exe not found (legacy fallback)`);
    return;
  }
  spawn(exe, [], { detached: true, stdio: "ignore" }).unref();
}

function openAsExtension(id: string): void {
  openExtension(id);
}

export const COMMANDS: InternalCommand[] = [
  // Dashboard — встроенный shell view (не extension), открывается в
  // отдельном BrowserWindow через dashboard-window.ts.
  {
    id: "dashboard:open",
    title: "Открыть таблицу данных",
    subtitle: "Просмотр объектов ARK",
    category: "open",
    exec: () => openDashboardWindow(),
  },
  {
    id: "settings:open",
    title: "Открыть настройки",
    subtitle: "Kepler",
    category: "open",
    exec: () => openSettings(),
  },
  // Phase 4 migrated apps — открываются как Vue extension'ы внутри Kepler.
  {
    id: "delphi:open",
    title: "Открыть Delphi",
    subtitle: "Задачи",
    category: "open",
    icon: () => extensionIconDataUri("delphi"),
    exec: () => openAsExtension("delphi"),
  },
  {
    id: "horologion:open",
    title: "Открыть Horologion",
    subtitle: "Pomodoro + трекер времени",
    category: "open",
    icon: () => extensionIconDataUri("horologion"),
    exec: () => openAsExtension("horologion"),
  },
  {
    id: "arrancador:open",
    title: "Открыть Arrancador",
    subtitle: "Игровая библиотека",
    category: "open",
    icon: () => extensionIconDataUri("arrancador"),
    exec: () => openAsExtension("arrancador"),
  },
  // Eden намеренно НЕ мигрирован в Phase 4 — остаётся standalone .exe.
  {
    id: "eden:open",
    title: "Открыть Eden",
    subtitle: "Заметки (legacy standalone)",
    category: "open",
    exec: () => openAppExe("eden"),
  },
];

export function findCommand(id: string): InternalCommand | undefined {
  return COMMANDS.find((c) => c.id === id);
}
