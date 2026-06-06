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
import { existsSync, readFileSync, readdirSync } from "node:fs";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const shellRoot = path.resolve(__dirname, "..");
const extensionsRoot = path.resolve(shellRoot, "..", "extensions");
const shellDevPort = 5173;

// --- .env.local loader (per-worktree dev slot override) ---------------------
// `shell/.env.local` (gitignored) задаёт `KEPLER_INSTANCE=dev-<slug>` для
// конкретного worktree. instance.ts резолвит slot и derive'ит userData /
// dataDir / productName. Без файла KEPLER_INSTANCE не set → default "dev".
//
// Minimal parser (без новых deps): KEY=VALUE по строке, # для комментариев,
// пустые строки skip. Никакой shell interpolation, никаких quote'ов.
function loadDotenvLocal() {
  const file = path.join(shellRoot, ".env.local");
  if (!existsSync(file)) return;
  try {
    const content = readFileSync(file, "utf8");
    for (const rawLine of content.split(/\r?\n/)) {
      const line = rawLine.trim();
      if (!line || line.startsWith("#")) continue;
      const eq = line.indexOf("=");
      if (eq <= 0) continue;
      const key = line.slice(0, eq).trim();
      let value = line.slice(eq + 1).trim();
      // Допускаем `KEY="value"` / `KEY='value'` чтобы не озадачивать
      // пользователей привычкой из dotenv.
      if (
        (value.startsWith('"') && value.endsWith('"')) ||
        (value.startsWith("'") && value.endsWith("'"))
      ) {
        value = value.slice(1, -1);
      }
      // Process env wins (явный CLI override > .env.local). Это важно для
      // CI и manual overrides.
      if (process.env[key] === undefined) {
        process.env[key] = value;
      }
    }
    if (process.env.KEPLER_INSTANCE) {
      console.log(`[dev] KEPLER_INSTANCE=${process.env.KEPLER_INSTANCE} (from .env.local)`);
    }
  } catch (e) {
    console.error("[dev] .env.local parse failed:", e);
  }
}
loadDotenvLocal();

const children = [];

function readExtensionDevPorts() {
  const requestedIds = process.env.KEPLER_DEV_EXTENSIONS === "1" ? null : new Set(["akasha"]);
  return readdirSync(extensionsRoot, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .filter((d) => !requestedIds || requestedIds.has(d.name))
    .map((d) => {
      const manifestPath = path.join(extensionsRoot, d.name, "manifest.json");
      if (!existsSync(manifestPath)) return null;
      try {
        const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
        if (manifest.kind !== "vue" || !manifest.devPort) return null;
        return { label: d.name, port: Number(manifest.devPort) };
      } catch {
        return null;
      }
    })
    .filter(Boolean);
}

function isPortAvailable(port, host = "127.0.0.1") {
  return new Promise((resolve) => {
    const server = net.createServer();
    server.once("error", () => resolve(false));
    server.once("listening", () => {
      server.close(() => resolve(true));
    });
    server.listen({ port, host, exclusive: true });
  });
}

async function assertDevPortsAvailable() {
  const checks = [{ label: "shell", port: shellDevPort }, ...readExtensionDevPorts()];
  const busy = [];
  for (const check of checks) {
    if (!(await isPortAvailable(check.port))) {
      busy.push(check);
    }
  }
  if (busy.length === 0) return;

  console.error("[dev] required dev port(s) are already in use:");
  for (const check of busy) {
    console.error(`  - ${check.label}: http://127.0.0.1:${check.port}/`);
  }
  console.error("[dev] stop the previous Kosmos dev run before starting a new one.");
  process.exit(1);
}

await assertDevPortsAvailable();

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

// 1) Vite dev server'ы для extension'ов (HMR live-reload).
//
//    Akasha default-on: это маленький reader extension без editor autosave
//    footgun'а, и в активной разработке он должен hot-reload'иться из обычного
//    `bun run --cwd shell dev`.
//
//    Все extensions opt-in через `KEPLER_DEV_EXTENSIONS=1`. Это сохраняет
//    прежнюю защиту Eden: HMR трогает Editor.vue mid-typing, useEditor создаёт
//    новый editor instance, и напечатанный пользователем но не успевший в
//    autosave (debounce 800ms) контент может потеряться.
//
//    Когда сервера подняты — extension-host автоматически использует их через
//    TCP probe (см. resolveExtensionSource в extension-host.ts). Без живого
//    порта — fallback на one-shot dist build.
if (process.env.KEPLER_DEV_EXTENSIONS === "1") {
  startChild("dev-extensions", "node", ["scripts/dev-extensions.mjs"]);
} else {
  startChild("dev-extensions:akasha", "node", ["scripts/dev-extensions.mjs", "--only", "akasha"]);
}

// 2) Shell renderer Vite + Electron (KEPLER_DEV=1 → main process знает что
//    мы в dev-сессии, не загружает production-mode пути).
startChild(
  "kepler-shell",
  "vite",
  [
    "--host",
    "127.0.0.1",
    "--port",
    String(shellDevPort),
    "--strictPort",
    "--configLoader",
    "native",
  ],
  {
    KEPLER_DEV: "1",
  },
);
