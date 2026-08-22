// Orchestrator для `bun run dev`: поднимает Vite renderer и Electron shell.
// Backend должен быть собран до запуска этого скрипта (см. package.json `dev`).

import { spawn, spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const shellRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(shellRoot, "..", "..");
const defaultShellDevPort = 9912;

// --- .env.local loader (per-worktree dev slot override) ---------------------
// `platform/desktop/.env.local` (gitignored) задаёт `KEPLER_INSTANCE=dev-<slug>` для
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

const shellDevPort = Number(process.env.KOSMOS_SHELL_DEV_PORT ?? defaultShellDevPort);
if (!Number.isInteger(shellDevPort) || shellDevPort < 1 || shellDevPort > 65535) {
  console.error(`[dev] invalid KOSMOS_SHELL_DEV_PORT=${process.env.KOSMOS_SHELL_DEV_PORT}`);
  process.exit(1);
}

const children = [];

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
  const checks = [{ label: "shell", port: shellDevPort }];
  const labelsByPort = new Map();
  for (const check of checks) {
    const previous = labelsByPort.get(check.port);
    if (previous) {
      console.error(`[dev] duplicate dev port ${check.port}: ${previous} and ${check.label}`);
      process.exit(1);
    }
    labelsByPort.set(check.port, check.label);
  }
  const busy = [];
  for (const check of checks) {
    if (await isPortAvailable(check.port)) {
      continue;
    }
    if (tryReleaseStaleDevPort(check.port)) {
      await new Promise((resolve) => setTimeout(resolve, 300));
      if (await isPortAvailable(check.port)) {
        continue;
      }
    }
    busy.push(check);
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

function psSingleQuoted(value) {
  return `'${String(value).replaceAll("'", "''")}'`;
}

function tryReleaseStaleDevPort(port) {
  if (process.platform !== "win32") return false;
  const script = `
$port = ${Number(port)}
$repo = ${psSingleQuoted(repoRoot)}
$conn = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $conn) { exit 0 }
$processInfo = Get-CimInstance Win32_Process -Filter "ProcessId=$($conn.OwningProcess)"
if (-not $processInfo) { exit 1 }
$cmd = [string]$processInfo.CommandLine
$cmdLower = $cmd.ToLowerInvariant()
$repoLower = $repo.ToLowerInvariant()
if ($cmdLower.Contains($repoLower) -and $cmdLower.Contains("vite")) {
  taskkill /PID $conn.OwningProcess /T /F | Out-Null
  exit 0
}
exit 1
`;
  const result = spawnSync("powershell.exe", ["-NoProfile", "-Command", script], {
    stdio: "ignore",
    windowsHide: true,
  });
  if (result.status !== 0) return false;
  return true;
}

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

function killProcessTree(child, signal) {
  if (child.killed) return;
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/T", "/F"], {
      stdio: "ignore",
      windowsHide: true,
    });
    return;
  }
  try {
    child.kill(signal);
  } catch {
    // ignore
  }
}

function shutdown(signal) {
  for (const { child } of children) {
    killProcessTree(child, signal);
  }
  // Дать процессам 500мс на graceful exit, потом force-exit.
  setTimeout(() => process.exit(0), 500);
}

process.on("SIGINT", () => shutdown("SIGINT"));
process.on("SIGTERM", () => shutdown("SIGTERM"));

// Shell renderer Vite + Electron (KEPLER_DEV=1 → main process знает что
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
    // Installed Kosmos may already own the fixed LAN-sync port 21531.
    // Opt in explicitly with KEPLER_SKIP_SYNC=0 when developing sync itself.
    KEPLER_SKIP_SYNC: process.env.KEPLER_SKIP_SYNC ?? "1",
  },
);
