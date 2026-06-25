import { ipcMain, shell } from "electron";
import { existsSync, mkdirSync, readdirSync, statSync, unlinkSync } from "node:fs";
import path from "node:path";
import { keplerDataDir } from "./data-dir";
import { safeHandle } from "./ipc-safe";

function crashesDirPath(): string {
  return path.join(keplerDataDir(), "crashes");
}

function listCrashLogs(): Array<{ name: string; size: number; mtime: string }> {
  const dir = crashesDirPath();
  if (!existsSync(dir)) return [];
  try {
    return readdirSync(dir)
      .filter((f) => f.endsWith(".log") || f.endsWith(".dmp"))
      .map((f) => {
        const stats = statSync(path.join(dir, f));
        return {
          name: f,
          size: stats.size,
          mtime: new Date(stats.mtimeMs).toISOString(),
        };
      })
      .sort((a, b) => b.mtime.localeCompare(a.mtime));
  } catch (e) {
    console.error("[kepler-shell] listCrashLogs failed:", e);
    return [];
  }
}

export function registerMainCrashesIpc(): void {
  safeHandle("kepler:crashes:list", async () => listCrashLogs());

  ipcMain.handle("kepler:crashes:openFolder", async () => {
    const dir = crashesDirPath();
    try {
      mkdirSync(dir, { recursive: true });
    } catch (e) {
      console.error("[kepler-shell] crashes:openFolder mkdir failed:", e);
    }
    const result = await shell.openPath(dir);
    if (result) {
      console.error("[kepler-shell] crashes:openFolder error:", result);
    }
  });

  ipcMain.handle("kepler:crashes:clear", () => {
    const dir = crashesDirPath();
    if (!existsSync(dir)) return { removed: 0 };
    let removed = 0;
    try {
      for (const f of readdirSync(dir)) {
        try {
          unlinkSync(path.join(dir, f));
          removed += 1;
        } catch (e) {
          console.error(`[kepler-shell] crashes:clear failed for ${f}:`, e);
        }
      }
    } catch (e) {
      console.error("[kepler-shell] crashes:clear readdir failed:", e);
    }
    return { removed };
  });
}
