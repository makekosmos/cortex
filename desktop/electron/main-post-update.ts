import { app, type BrowserWindow } from "electron";
import { existsSync, readFileSync, unlinkSync } from "node:fs";
import path from "node:path";
import { keplerDataDir } from "./data-dir";
import { resolveInstance } from "./instance";

interface PostUpdateLauncher {
  getMainWindow(): BrowserWindow | null;
  showLauncher(): void;
}

export function handlePostUpdateFirstLaunch(launcher: PostUpdateLauncher): void {
  try {
    const flag = findPostUpdateFlag();
    if (!existsSync(flag)) return;
    try {
      readFileSync(flag, "utf8");
    } catch {
      /* ignore — флаг всё равно удаляем */
    }
    unlinkSync(flag);
    const newVersion = app.getVersion();
    launcher.showLauncher();
    setTimeout(() => {
      const mainWindow = launcher.getMainWindow();
      if (!mainWindow || mainWindow.isDestroyed()) return;
      try {
        mainWindow.webContents.send("kepler:post-update", { version: newVersion });
      } catch {
        /* dead webContents — skip */
      }
    }, 300);
  } catch (e) {
    console.warn("[kepler-shell] post-update flag handling failed:", e);
  }
}

function findPostUpdateFlag(): string {
  const userDataFlag = path.join(resolveInstance().userDataDir, "post-update.flag");
  if (existsSync(userDataFlag)) return userDataFlag;
  return path.join(keplerDataDir(), "post-update.flag");
}
