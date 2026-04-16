import { contextBridge, ipcRenderer } from "electron";
import type {
  DashboardLoadOptions,
  DashboardPlatform,
  DashboardSnapshot,
} from "@shared/analytics";

contextBridge.exposeInMainWorld("dashboardApi", {
  loadSnapshot: (options?: DashboardLoadOptions): Promise<DashboardSnapshot> =>
    ipcRenderer.invoke("dashboard:get-snapshot", options),
  chooseDatabase: (): Promise<DashboardSnapshot> =>
    ipcRenderer.invoke("dashboard:choose-database"),
  resetDatabase: (): Promise<DashboardSnapshot> =>
    ipcRenderer.invoke("dashboard:reset-database"),
  getDefaultDbPath: (): Promise<string> => ipcRenderer.invoke("dashboard:get-default-db-path"),
  getPlatform: (): Promise<DashboardPlatform> => ipcRenderer.invoke("dashboard:get-platform"),
  minimize: () => ipcRenderer.send("window:minimize"),
  maximize: () => ipcRenderer.send("window:maximize"),
  close: () => ipcRenderer.send("window:close"),
});
