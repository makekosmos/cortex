// Static command registry — open-commands ("Открыть <App>") теперь открывают
// апки как Vue extension'ы внутри Kepler через extension-host (Phase 4).
// Spawn .exe path остаётся fallback'ом для Eden (она ещё standalone Electron).
//
// Action-commands (Pomodoro start, create note и т.п.) приходят dynamic от
// running extension'ов через kepler-backend command bus.

import "./extension-host";

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

async function openAsExtension(id: string): Promise<void> {
  const { openExtension } = await import("./extension-host");
  openExtension(id);
}

export const COMMANDS: InternalCommand[] = [
  // Phase 4 migrated apps — открываются как Vue extension'ы внутри Kepler.
  {
    id: "dashboard:open",
    title: "Открыть Dashboard",
    subtitle: "Аналитика",
    category: "open",
    exec: () => openAsExtension("dashboard"),
  },
  {
    id: "delphi:open",
    title: "Открыть Delphi",
    subtitle: "Задачи",
    category: "open",
    exec: () => openAsExtension("delphi"),
  },
  {
    id: "horologion:open",
    title: "Открыть Horologion",
    subtitle: "Pomodoro + трекер времени",
    category: "open",
    exec: () => openAsExtension("horologion"),
  },
  {
    id: "arrancador:open",
    title: "Открыть Arrancador",
    subtitle: "Игровая библиотека",
    category: "open",
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
