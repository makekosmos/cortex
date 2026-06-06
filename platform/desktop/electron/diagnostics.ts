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

import { app, ipcMain, dialog, shell } from "electron";
import {
  existsSync,
  mkdirSync,
  writeFileSync,
  copyFileSync,
  readdirSync,
  statSync,
  rmSync,
  unlinkSync,
} from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { keplerDataDir } from "./data-dir";
import { keplerLog } from "./logging";
import { resolveInstance } from "./instance";
import { listInstalledUserExtensions } from "./extension-installer";

const LOGS_KEEP_DAYS = 7;
const CRASHES_KEEP_DAYS = 30;

interface BundleResult {
  ok: boolean;
  zipPath?: string;
  error?: string;
}

function copyRecentFiles(srcDir: string, destDir: string, keepDays: number): number {
  if (!existsSync(srcDir)) return 0;
  const cutoff = Date.now() - keepDays * 24 * 60 * 60 * 1000;
  mkdirSync(destDir, { recursive: true });
  let copied = 0;
  for (const entry of readdirSync(srcDir, { withFileTypes: true })) {
    if (!entry.isFile()) continue;
    const fullPath = path.join(srcDir, entry.name);
    try {
      const stats = statSync(fullPath);
      if (stats.mtimeMs < cutoff) continue;
      copyFileSync(fullPath, path.join(destDir, entry.name));
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

function buildInstalledExtensionsJson(): string {
  try {
    const installed = listInstalledUserExtensions();
    return JSON.stringify(installed, null, 2);
  } catch (e) {
    keplerLog.warn("diagnostics", "listInstalledUserExtensions failed", { err: String(e) });
    return JSON.stringify({ error: String(e) }, null, 2);
  }
}

async function createBundleZip(): Promise<BundleResult> {
  const stagingRoot = app.getPath("temp");
  const ts = new Date().toISOString().replace(/[:.]/g, "-");
  const stagingDir = path.join(stagingRoot, `kepler-bug-${ts}`);
  const zipPath = path.join(stagingRoot, `kepler-bug-${ts}.zip`);

  mkdirSync(stagingDir, { recursive: true });

  try {
    // 1. Логи.
    const logsCount = copyRecentFiles(
      path.join(keplerDataDir(), "logs"),
      path.join(stagingDir, "logs"),
      LOGS_KEEP_DAYS,
    );
    keplerLog.info("diagnostics", "logs copied", { count: logsCount });

    // 2. Crashes.
    const crashesCount = copyRecentFiles(
      path.join(keplerDataDir(), "crashes"),
      path.join(stagingDir, "crashes"),
      CRASHES_KEEP_DAYS,
    );
    keplerLog.info("diagnostics", "crashes copied", { count: crashesCount });

    // 3. Metadata.
    writeFileSync(path.join(stagingDir, "versions.json"), buildVersionsJson(), "utf8");
    writeFileSync(
      path.join(stagingDir, "installed-extensions.json"),
      buildInstalledExtensionsJson(),
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
    const res = spawnSync("powershell.exe", ["-NoProfile", "-Command", psCmd], {
      windowsHide: true,
    });
    if (res.status !== 0) {
      const stderr = res.stderr?.toString() ?? "(no stderr)";
      keplerLog.error("diagnostics", "Compress-Archive failed", { stderr, status: res.status });
      return { ok: false, error: `Compress-Archive failed: ${stderr}` };
    }

    // 5. Очистка staging dir — ZIP отдельно.
    try {
      rmSync(stagingDir, { recursive: true, force: true });
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
    copyFileSync(bundle.zipPath, result.filePath);
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
