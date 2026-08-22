import { ipcMain } from "electron";
import {
  check as checkForUpdates,
  getState as getUpdateState,
  install as installUpdate,
} from "./autoupdater-host";

export function registerMainDataSettingsIpc(): void {
  ipcMain.handle("kepler:settings:update:check", () => checkForUpdates());
  ipcMain.handle("kepler:settings:update:install", () => {
    installUpdate();
    return true;
  });
  ipcMain.handle("kepler:settings:update:state", () => getUpdateState());
}
