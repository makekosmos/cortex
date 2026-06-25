import { BrowserWindow, ipcMain } from "electron";
import {
  installFromPath,
  listBackups,
  listInstalledUserExtensions,
  previewSource,
  revertExtension,
  uninstallExtension,
} from "./extension-installer";

function notifyCommandsChanged(): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      try {
        win.webContents.send("kepler:commands:updated");
      } catch {}
    }
  }
}

export function registerExtensionInstallerIpc(): void {
  ipcMain.handle("kepler:extension:install:preview", async (_e, sourcePath: string) => {
    if (typeof sourcePath !== "string") {
      throw new Error("install:preview: sourcePath must be a string");
    }
    return previewSource(sourcePath);
  });

  ipcMain.handle("kepler:extension:install:do", async (_e, sourcePath: string) => {
    if (typeof sourcePath !== "string") {
      throw new Error("install:do: sourcePath must be a string");
    }
    const result = await installFromPath(sourcePath);
    notifyCommandsChanged();
    return result;
  });

  ipcMain.handle("kepler:extension:installed:list", async () => {
    return listInstalledUserExtensions();
  });

  ipcMain.handle("kepler:extension:revert", async (_e, id: string, timestamp?: string) => {
    if (typeof id !== "string") {
      throw new Error("revert: id must be a string");
    }
    const result = await revertExtension(id, timestamp);
    notifyCommandsChanged();
    return result;
  });

  ipcMain.handle("kepler:extension:backups:list", async (_e, id: string) => {
    if (typeof id !== "string") {
      throw new Error("backups:list: id must be a string");
    }
    return listBackups(id);
  });

  ipcMain.handle("kepler:extension:uninstall", async (_e, id: string) => {
    if (typeof id !== "string") {
      throw new Error("uninstall: id must be a string");
    }
    const result = await uninstallExtension(id);
    notifyCommandsChanged();
    return result;
  });
}
