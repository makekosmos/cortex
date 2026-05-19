// Orchestrator для `bun run dev`: параллельно поднимает:
//   1) Vite dev server'ы всех Vue extension'ов (через dev-extensions.mjs) —
//      даёт HMR в Eden / Delphi / Horologion / Arrancador.
//   2) Vite renderer для shell (KEPLER_DEV=1 — Electron подхватывает
//      hot reload главного окна + extension dev mode загружается с
//      http://localhost:<devPort>/ при включённом Settings → Developer mode).
//
// Backend и one-shot build extension'ов должны быть выполнены до запуска
// этого скрипта (см. package.json `dev`). SIGINT/SIGTERM пробрасывается
// children, чтобы Ctrl+C корректно убивал всё дерево.

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const shellRoot = path.resolve(__dirname, "..");

const children = [];

function startChild(name, cmd, args, env = {}) {
  const child = spawn(cmd, args, {
    cwd: shellRoot,
    stdio: "inherit",
    shell: true,
    env: { ...process.env, ...env },
  });
  child.on("exit", (code, signal) => {
    if (signal !== "SIGTERM" && signal !== "SIGINT") {
      console.warn(`[dev] ${name} exited code=${code} signal=${signal}`);
    }
  });
  children.push({ name, child });
  return child;
}

function shutdown(signal) {
  for (const { child } of children) {
    if (!child.killed) {
      try {
        child.kill(signal);
      } catch {
        // ignore
      }
    }
  }
  // Дать процессам 500мс на graceful exit, потом force-exit.
  setTimeout(() => process.exit(0), 500);
}

process.on("SIGINT", () => shutdown("SIGINT"));
process.on("SIGTERM", () => shutdown("SIGTERM"));

// 1) Vite dev server'ы для extension'ов (HMR live-reload). По дефолту ВЫКЛЮЧЕНО.
//    Включил было default-on (2026-05-19), но обнаружилось что HMR трогает
//    Editor.vue mid-typing: при любом edit'е файла в working tree Vite
//    реклоадит Eden window, useEditor создаёт новый editor instance, и
//    напечатанный пользователем но не успевший в autosave (debounce 800ms)
//    контент теряется. Для агента, активно редактирующего код во время того
//    как user тестит, это destructive.
//
//    Opt-in: `KEPLER_DEV_EXTENSIONS=1`. Когда сервера подняты — extension-host
//    автоматически использует их через TCP probe (см. resolveExtensionSource
//    в extension-host.ts). Без них — fallback на one-shot dist build (быстрый
//    rebuild через `bun run --cwd shell build:extensions`).
if (process.env.KEPLER_DEV_EXTENSIONS === "1") {
  startChild("dev-extensions", "node", ["scripts/dev-extensions.mjs"]);
}

// 2) Shell renderer Vite + Electron (KEPLER_DEV=1 → main process знает что
//    мы в dev-сессии, не загружает production-mode пути).
startChild("kepler-shell", "vite", ["--configLoader", "native"], {
  KEPLER_DEV: "1",
});
