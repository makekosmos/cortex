// Phase 4 bug-detection: diagnostics bundle ZIP.
//
// IPC `kepler:diagnostics:bundle` — создаёт ZIP с:
//   - последние N дней logs/ (per-slot dir),
//   - crashes/ (если есть),
//   - versions.json (kepler version, electron version, OS info),
//   - installed-extensions.json (id + version каждого).
//
// Что НЕ включаем (privacy / size):
//   - ark.db — содержит пользовательские объекты (notes / tasks / etc).
//   - extensions-data/ — user content extension'ов.
//   - secrets — env, лок-токены.
//
// Renderer:
//   const zipPath = await window.kepler.diagnostics.bundle();
//   // → ZIP создан в temp dir, путь возвращён. Renderer показывает
//   // showSaveDialog и копирует / переименовывает.

import { app, BrowserWindow, contentTracing, ipcMain, dialog, shell } from "electron";
import { existsSync, mkdirSync, unlinkSync } from "node:fs";
import { promises as fs } from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { keplerDataDir } from "./data-dir";
import { keplerLog } from "./logging";
import { resolveInstance } from "./instance";
import { listInstalledUserExtensions } from "./extension-installer";
import {
  runWindowMoveBenchmark,
  type WindowMoveBenchmarkInput,
} from "./diagnostics-window-benchmark";

const LOGS_KEEP_DAYS = 7;
const CRASHES_KEEP_DAYS = 30;

interface BundleResult {
  ok: boolean;
  zipPath?: string;
  error?: string;
}

async function copyRecentFiles(srcDir: string, destDir: string, keepDays: number): Promise<number> {
  if (!existsSync(srcDir)) return 0;
  const cutoff = Date.now() - keepDays * 24 * 60 * 60 * 1000;
  await fs.mkdir(destDir, { recursive: true });
  let copied = 0;
  for (const entry of await fs.readdir(srcDir, { withFileTypes: true })) {
    if (!entry.isFile()) continue;
    const fullPath = path.join(srcDir, entry.name);
    try {
      const stats = await fs.stat(fullPath);
      if (stats.mtimeMs < cutoff) continue;
      await fs.copyFile(fullPath, path.join(destDir, entry.name));
      copied++;
    } catch {
      // Skip unreadable file
    }
  }
  return copied;
}

function buildVersionsJson(): string {
  const versions: Record<string, unknown> = {
    kepler: app.getVersion(),
    electron: process.versions.electron,
    node: process.versions.node,
    chromium: process.versions.chrome,
    v8: process.versions.v8,
    platform: process.platform,
    arch: process.arch,
    osRelease: require("node:os").release(),
    slot: resolveInstance().slot,
    capturedAt: new Date().toISOString(),
  };
  return JSON.stringify(versions, null, 2);
}

async function buildInstalledExtensionsJson(): Promise<string> {
  try {
    const installed = await listInstalledUserExtensions();
    return JSON.stringify(installed, null, 2);
  } catch (e) {
    keplerLog.warn("diagnostics", "listInstalledUserExtensions failed", { err: String(e) });
    return JSON.stringify({ error: String(e) }, null, 2);
  }
}

function windowSnapshot(win: BrowserWindow) {
  return {
    id: win.id,
    title: win.getTitle(),
    visible: win.isVisible(),
    minimized: win.isMinimized(),
    alwaysOnTop: win.isAlwaysOnTop(),
    bounds: win.getBounds(),
    pid: win.webContents.getOSProcessId(),
    url: win.webContents.getURL(),
  };
}

async function spawnAsync(
  command: string,
  args: string[],
): Promise<{ status: number; stderr: string }> {
  return new Promise((resolve) => {
    let stderr = "";
    const child = spawn(command, args, { windowsHide: true });
    child.stderr?.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.on("exit", (status) => {
      resolve({ status: status ?? 1, stderr });
    });
  });
}

async function createBundleZip(): Promise<BundleResult> {
  const stagingRoot = app.getPath("temp");
  const ts = new Date().toISOString().replace(/[:.]/g, "-");
  const stagingDir = path.join(stagingRoot, `kepler-bug-${ts}`);
  const zipPath = path.join(stagingRoot, `kepler-bug-${ts}.zip`);

  await fs.mkdir(stagingDir, { recursive: true });

  try {
    // 1. Логи.
    const logsCount = await copyRecentFiles(
      path.join(keplerDataDir(), "logs"),
      path.join(stagingDir, "logs"),
      LOGS_KEEP_DAYS,
    );
    keplerLog.info("diagnostics", "logs copied", { count: logsCount });

    // 2. Crashes.
    const crashesCount = await copyRecentFiles(
      path.join(keplerDataDir(), "crashes"),
      path.join(stagingDir, "crashes"),
      CRASHES_KEEP_DAYS,
    );
    keplerLog.info("diagnostics", "crashes copied", { count: crashesCount });

    // 3. Metadata.
    await fs.writeFile(path.join(stagingDir, "versions.json"), buildVersionsJson(), "utf8");
    await fs.writeFile(
      path.join(stagingDir, "installed-extensions.json"),
      await buildInstalledExtensionsJson(),
      "utf8",
    );

    // 4. ZIP via PowerShell. Windows-only — у нас Windows-only product.
    if (existsSync(zipPath)) {
      try {
        unlinkSync(zipPath);
      } catch {
        // ignore
      }
    }
    const psCmd = `Compress-Archive -Path '${stagingDir}\\*' -DestinationPath '${zipPath}' -Force`;
    const res = await spawnAsync("powershell.exe", ["-NoProfile", "-Command", psCmd]);
    if (res.status !== 0) {
      const stderr = res.stderr ?? "(no stderr)";
      keplerLog.error("diagnostics", "Compress-Archive failed", { stderr, status: res.status });
      return { ok: false, error: `Compress-Archive failed: ${stderr}` };
    }

    // 5. Очистка staging dir — ZIP отдельно.
    try {
      await fs.rm(stagingDir, { recursive: true, force: true });
    } catch (e) {
      keplerLog.warn("diagnostics", "staging cleanup failed", { err: String(e) });
    }

    keplerLog.info("diagnostics", "bundle created", { zipPath });
    return { ok: true, zipPath };
  } catch (e) {
    keplerLog.error("diagnostics", "bundle failed", { err: String(e) });
    return { ok: false, error: String(e) };
  }
}

// --- IPC ---------------------------------------------------------------------

ipcMain.handle("kepler:diagnostics:bundle", async (): Promise<BundleResult> => {
  return createBundleZip();
});

/**
 * Создаёт ZIP и сразу показывает saveDialog пользователю для финального
 * destination. После сохранения копирует туда и удаляет staging ZIP в temp.
 *
 * Возвращает final path или null если пользователь отменил.
 */
ipcMain.handle("kepler:diagnostics:bundle-save", async (): Promise<string | null> => {
  const bundle = await createBundleZip();
  if (!bundle.ok || !bundle.zipPath) return null;

  const defaultName = path.basename(bundle.zipPath);
  const result = await dialog.showSaveDialog({
    title: "Сохранить bug report",
    defaultPath: defaultName,
    filters: [{ name: "ZIP archive", extensions: ["zip"] }],
  });

  if (result.canceled || !result.filePath) {
    try {
      unlinkSync(bundle.zipPath);
    } catch {
      // ignore
    }
    return null;
  }

  try {
    await fs.copyFile(bundle.zipPath, result.filePath);
    unlinkSync(bundle.zipPath);
    keplerLog.info("diagnostics", "bundle saved to user-chosen path", {
      path: result.filePath,
    });
    return result.filePath;
  } catch (e) {
    keplerLog.error("diagnostics", "bundle save copy failed", { err: String(e) });
    return null;
  }
});

ipcMain.handle("kepler:diagnostics:open-logs-folder", async (): Promise<void> => {
  const logsDir = keplerLog.logsDir();
  if (!existsSync(logsDir)) mkdirSync(logsDir, { recursive: true });
  await shell.openPath(logsDir);
});

ipcMain.handle("kepler:diagnostics:metrics", async () => {
  return {
    at: new Date().toISOString(),
    appMetrics: app.getAppMetrics(),
    gpuFeatureStatus: app.getGPUFeatureStatus(),
    gpuInfo: await app.getGPUInfo("basic").catch((e) => ({ error: String(e) })),
    windows: BrowserWindow.getAllWindows()
      .filter((win) => !win.isDestroyed())
      .map(windowSnapshot),
  };
});

ipcMain.handle("kepler:diagnostics:trace-start", async () => {
  await contentTracing.startRecording({
    included_categories: [
      "electron",
      "blink",
      "cc",
      "gpu",
      "toplevel",
      "disabled-by-default-v8.cpu_profiler",
    ],
  });
  return { ok: true };
});

ipcMain.handle("kepler:diagnostics:trace-stop", async (_event, outPath?: string) => {
  const tracePath = await contentTracing.stopRecording(outPath);
  return { path: tracePath };
});

ipcMain.handle(
  "kepler:diagnostics:window-move-benchmark",
  async (_event, input?: WindowMoveBenchmarkInput) => runWindowMoveBenchmark(input),
);
