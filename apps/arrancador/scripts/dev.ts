import { spawn, type ChildProcess } from "node:child_process";
import { createRequire } from "node:module";
import { watch } from "node:fs";
import { mkdir, stat } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { setTimeout as delay } from "node:timers/promises";

const ROOT = process.cwd();
const BUN = process.execPath;
const require = createRequire(import.meta.url);
const RENDERER_URL = "http://127.0.0.1:5173";
const MAIN_OUT = path.join(ROOT, "out", "main", "index.js");
const PRELOAD_OUT = path.join(ROOT, "out", "preload", "index.js");
const ELECTRON_BIN = path.join(
  path.dirname(require.resolve("electron/package.json")),
  "dist",
  process.platform === "win32" ? "electron.exe" : "electron",
);

const childProcesses = new Set<ChildProcess>();
let electronProcess: ChildProcess | null = null;
let shuttingDown = false;
let restartTimer: NodeJS.Timeout | null = null;
let electronStartPromise: Promise<void> | null = null;

function log(message: string) {
  process.stdout.write(`[dev] ${message}\n`);
}

function spawnChild(args: string[], extraEnv: Record<string, string> = {}) {
  const child = spawn(BUN, args, {
    cwd: ROOT,
    env: {
      ...process.env,
      ...extraEnv,
    },
    stdio: "inherit",
  });

  childProcesses.add(child);
  child.once("exit", () => {
    childProcesses.delete(child);
  });

  return child;
}

async function exists(target: string) {
  try {
    await stat(target);
    return true;
  } catch {
    return false;
  }
}

async function waitForRenderer(timeoutMs = 60_000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const response = await fetch(RENDERER_URL);
      if (response.ok) {
        return;
      }
    } catch {
      // Retry until ready.
    }
    await delay(300);
  }

  throw new Error("Vite dev server did not become ready in time");
}

async function isRendererAvailable() {
  try {
    const response = await fetch(RENDERER_URL);
    return response.ok;
  } catch {
    return false;
  }
}

async function waitForArtifacts(timeoutMs = 60_000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if ((await exists(MAIN_OUT)) && (await exists(PRELOAD_OUT))) {
      return;
    }
    await delay(150);
  }

  throw new Error("Electron main/preload bundles did not become ready in time");
}

async function killProcessTree(child: ChildProcess | null) {
  if (!child?.pid) {
    return;
  }

  if (process.platform === "win32") {
    await new Promise<void>((resolve) => {
      const killer = spawn("taskkill", ["/PID", String(child.pid), "/T", "/F"], {
        stdio: "ignore",
      });
      killer.once("exit", () => resolve());
      killer.once("error", () => resolve());
    });
    return;
  }

  child.kill("SIGTERM");
  await delay(500);
  if (!child.killed) {
    child.kill("SIGKILL");
  }
}

async function startElectron() {
  if (electronStartPromise) {
    return await electronStartPromise;
  }

  electronStartPromise = (async () => {
    await waitForRenderer();
    await waitForArtifacts();

    const child = spawn(ELECTRON_BIN, ["."], {
      cwd: ROOT,
      env: {
        ...process.env,
        ELECTRON_RENDERER_URL: RENDERER_URL,
      },
      stdio: "inherit",
    });
    electronProcess = child;
    childProcesses.add(child);
    child.once("exit", () => {
      childProcesses.delete(child);
    });

    child.once("exit", (code) => {
      if (!shuttingDown && code && code !== 0) {
        log(`electron exited with code ${code}`);
      }
      if (electronProcess === child) {
        electronProcess = null;
      }
    });
  })();

  try {
    await electronStartPromise;
  } finally {
    electronStartPromise = null;
  }
}

async function restartElectron() {
  if (shuttingDown) {
    return;
  }

  const current = electronProcess;
  electronProcess = null;
  await killProcessTree(current);
  await delay(250);
  await startElectron();
}

function scheduleRestart() {
  if (shuttingDown) {
    return;
  }

  if (restartTimer) {
    clearTimeout(restartTimer);
  }

  restartTimer = setTimeout(() => {
    restartTimer = null;
    void restartElectron();
  }, 200);
}

function watchOutput(target: string) {
  const directory = path.dirname(target);
  const fileName = path.basename(target);

  const watcher = watch(directory, (_eventType, changedFile) => {
    if (changedFile && changedFile.toString() === fileName) {
      scheduleRestart();
    }
  });

  watcher.once("error", (error) => {
    if (!shuttingDown) {
      process.stderr.write(`[dev] watch error for ${target}: ${String(error)}\n`);
    }
  });

  return watcher;
}

async function shutdown(code = 0) {
  if (shuttingDown) {
    return;
  }

  shuttingDown = true;
  if (restartTimer) {
    clearTimeout(restartTimer);
    restartTimer = null;
  }

  await killProcessTree(electronProcess);
  electronProcess = null;

  for (const child of [...childProcesses]) {
    await killProcessTree(child);
  }

  process.exit(code);
}

async function main() {
  await mkdir(path.dirname(MAIN_OUT), { recursive: true });
  await mkdir(path.dirname(PRELOAD_OUT), { recursive: true });

  if (await isRendererAvailable()) {
    log(`reusing existing renderer at ${RENDERER_URL}`);
  } else {
    spawnChild([
      "x",
      "vite",
      "--host",
      "127.0.0.1",
      "--port",
      "5173",
      "--strictPort",
    ]);
  }
  spawnChild([
    "build",
    "./electron/main.ts",
    "--outfile=out/main/index.js",
    "--target=node",
    "--format=esm",
    "-e",
    "electron",
    "-e",
    "better-sqlite3",
    "--watch",
  ]);
  spawnChild([
    "build",
    "./electron/preload.ts",
    "--outfile=out/preload/index.js",
    "--target=node",
    "--format=esm",
    "-e",
    "electron",
    "--watch",
  ]);

  await startElectron();

  const mainWatcher = watchOutput(MAIN_OUT);
  const preloadWatcher = watchOutput(PRELOAD_OUT);

  process.once("SIGINT", () => {
    mainWatcher.close();
    preloadWatcher.close();
    void shutdown(0);
  });
  process.once("SIGTERM", () => {
    mainWatcher.close();
    preloadWatcher.close();
    void shutdown(0);
  });
}

void main().catch(async (error) => {
  process.stderr.write(`[dev] ${error instanceof Error ? error.stack ?? error.message : String(error)}\n`);
  await shutdown(1);
});
