// Static command registry — open-commands ("Открыть <App>"), которые
// kepler-shell исполняет локально через `spawn(exe)`. Это единственные
// статические команды launcher'а.
//
// Action-commands (Pomodoro start, create note и т.п.) теперь приходят
// dynamic от running апок через kepler-backend (ArkClient.commands.list()).
// Здесь их нет — они регистрируются апками и исполняются на стороне апки
// после broadcast'а `command_invoked` от backend'а.

import "./extension-host";

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// ESM shim — __dirname / __filename не определены в Node ESM bundles
// (electron-vite собирает main как ESM). Без этого resolveAppExe падает
// с ReferenceError: __dirname is not defined.
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export interface InternalCommand {
  id: string;
  title: string;
  subtitle: string;
  category: "open" | "action";
  exec: () => Promise<void> | void;
}

function resolveAppExe(appLower: string): string | null {
  const cap = appLower.charAt(0).toUpperCase() + appLower.slice(1);
  const candidates = [
    // Production install (после electron-builder NSIS/MSI)
    path.join(process.env.LOCALAPPDATA ?? "", "Kosmos", cap, `${cap}.exe`),
    path.join(process.env.LOCALAPPDATA ?? "", "Programs", cap, `${cap}.exe`),
    path.join(process.env.PROGRAMFILES ?? "", cap, `${cap}.exe`),
    path.join(process.env.PROGRAMFILES ?? "", "Kosmos", cap, `${cap}.exe`),
    // Dev: release/ внутри workspace
    path.resolve(__dirname, "..", "..", "..", "..", "apps", appLower, "release", `${cap}.exe`),
    path.resolve(__dirname, "..", "..", "..", "..", "apps", appLower, "ts", "release", `${cap}.exe`),
  ];
  for (const c of candidates) {
    if (c && existsSync(c)) return c;
  }
  return null;
}

function openApp(appLower: string): void {
  const exe = resolveAppExe(appLower);
  if (!exe) {
    console.warn(`[kepler-shell] ${appLower}.exe not found in known paths`);
    return;
  }
  spawn(exe, [], { detached: true, stdio: "ignore" }).unref();
}

export const COMMANDS: InternalCommand[] = [
  {
    id: "eden:open",
    title: "Открыть Eden",
    subtitle: "Заметки",
    category: "open",
    exec: () => openApp("eden"),
  },
  {
    id: "delphi:open",
    title: "Открыть Delphi",
    subtitle: "Задачи",
    category: "open",
    exec: () => openApp("delphi"),
  },
  {
    id: "horologion:open",
    title: "Открыть Horologion",
    subtitle: "Pomodoro + трекер времени",
    category: "open",
    exec: () => openApp("horologion"),
  },
  {
    id: "arrancador:open",
    title: "Открыть Arrancador",
    subtitle: "Игровая библиотека",
    category: "open",
    exec: () => openApp("arrancador"),
  },
  {
    id: "dashboard:open",
    title: "Открыть Dashboard",
    subtitle: "Аналитика",
    category: "open",
    exec: () => openApp("dashboard"),
  },
  {
    id: "dashboard:extension:demo",
    title: "Открыть Dashboard (PoC extension)",
    subtitle: "Demo",
    category: "open",
    exec: async () => {
      // Lazy import чтобы не подгружать extension-host на старте если не вызвано
      const { openExtension } = await import("./extension-host");
      openExtension("dashboard");
    },
  },
];

export function findCommand(id: string): InternalCommand | undefined {
  return COMMANDS.find((c) => c.id === id);
}
