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

import { app, BrowserWindow, contentTracing, ipcMain } from "electron";
import { existsSync, unlinkSync } from "node:fs";
import { promises as fs } from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { keplerDataDir } from "./data-dir";
import { keplerLog } from "./logging";
import { resolveInstance } from "./instance";
import { readEmbeddedReleaseBomIdentity, type ReleaseBomIdentity } from "./release-bom-identity";
import {
  runWindowMoveBenchmark,
  type WindowMoveBenchmarkInput,
} from "./diagnostics-window-benchmark";
import {
  isSupportTextFile,
  MAX_SUPPORT_TEXT_FILE_BYTES,
  redactText,
  redactTextFile,
} from "./redaction";

const LOGS_KEEP_DAYS = 7;
const CRASHES_KEEP_DAYS = 30;
interface BundleResult {
  ok: boolean;
  zipPath?: string;
  error?: string;
}

interface VersionsSnapshot {
  kepler: string;
  electron: string;
  node: string;
  chromium: string;
  v8: string;
  platform: NodeJS.Platform;
  arch: string;
  osRelease: string;
  slot: string;
  capturedAt: string;
  bom?: ReleaseBomIdentity;
}

async function copyRecentTextFilesRedacted(
  srcDir: string,
  destDir: string,
  keepDays: number,
): Promise<number> {
  if (!existsSync(srcDir)) return 0;
  const cutoff = Date.now() - keepDays * 24 * 60 * 60 * 1000;
  await fs.mkdir(destDir, { recursive: true });
  let copied = 0;
  for (const entry of await fs.readdir(srcDir, { withFileTypes: true })) {
    if (!entry.isFile()) continue;
    if (!isSupportTextFile(entry.name)) continue;
    const fullPath = path.join(srcDir, entry.name);
    try {
      const stats = await fs.stat(fullPath);
      if (stats.mtimeMs < cutoff) continue;
      if (await copyRedactedTextFileBounded(fullPath, path.join(destDir, entry.name))) copied++;
    } catch {
      // Skip unreadable file
    }
  }
  return copied;
}

export async function copyRedactedTextFileBounded(
  srcPath: string,
  destPath: string,
): Promise<boolean> {
  try {
    const file = await fs.open(srcPath, "r");
    const buffer = Buffer.alloc(MAX_SUPPORT_TEXT_FILE_BYTES);
    let bytesRead = 0;
    try {
      ({ bytesRead } = await file.read(buffer, 0, buffer.length, 0));
    } finally {
      await file.close();
    }
    const redacted = redactTextFile(buffer.subarray(0, bytesRead).toString("utf8"));
    await fs.writeFile(
      destPath,
      Buffer.from(redacted, "utf8").subarray(0, MAX_SUPPORT_TEXT_FILE_BYTES),
    );
    return true;
  } catch {
    return false;
  }
}

function buildVersionsJson(): string {
  const bom = readEmbeddedReleaseBomIdentity();
  const versions: VersionsSnapshot = {
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
  if (bom) versions.bom = bom;
  return JSON.stringify(versions, null, 2);
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
    const logsCount = await copyRecentTextFilesRedacted(
      path.join(keplerDataDir(), "logs"),
      path.join(stagingDir, "logs"),
      LOGS_KEEP_DAYS,
    );
    keplerLog.info("diagnostics", "logs copied", { count: logsCount });

    // 2. Crashes.
    const crashesCount = await copyRecentTextFilesRedacted(
      path.join(keplerDataDir(), "crashes"),
      path.join(stagingDir, "crashes"),
      CRASHES_KEEP_DAYS,
    );
    keplerLog.info("diagnostics", "crashes copied", { count: crashesCount });

    // 3. Metadata.
    await fs.writeFile(path.join(stagingDir, "versions.json"), buildVersionsJson(), "utf8");
    const protocolUsagePath = path.join(keplerDataDir(), "protocol-usage.json");
    if (existsSync(protocolUsagePath)) {
      await copyRedactedTextFileBounded(
        protocolUsagePath,
        path.join(stagingDir, "protocol-usage.json"),
      );
    }

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
      const stderr = redactText(res.stderr ?? "(no stderr)");
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
