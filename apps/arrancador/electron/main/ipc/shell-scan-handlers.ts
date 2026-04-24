import {
  app,
  BrowserWindow,
  dialog,
  ipcMain,
  type OpenDialogOptions,
  shell,
} from "electron";
import {
  createScanCancellation,
  getRunningProcesses,
  scanExecutablesStream,
} from "../services/scan";
import type { WithRuntime } from "./types";

export interface ShellScanIpcDeps {
  withRuntime: WithRuntime;
  emitRendererEvent: (channel: string, payload: unknown) => void;
}

function getTargetWindow() {
  return BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
}

export function registerShellScanIpcHandlers(deps: ShellScanIpcDeps) {
  const { withRuntime } = deps;

  ipcMain.handle(
    "get_running_processes",
    withRuntime(async () => await getRunningProcesses()),
  );

  ipcMain.handle(
    "dialog_open",
    withRuntime(async (_runtime, payload: OpenDialogOptions) => {
      const window = getTargetWindow();
      const result = window
        ? await dialog.showOpenDialog(window, payload)
        : await dialog.showOpenDialog(payload);

      if (result.canceled) {
        return null;
      }

      if (payload.multiple) {
        return result.filePaths;
      }

      return result.filePaths[0] ?? null;
    }),
  );

  ipcMain.handle(
    "get_window_platform",
    withRuntime(async () => process.platform),
  );

  ipcMain.handle(
    "window_minimize",
    withRuntime(async () => {
      getTargetWindow()?.minimize();
    }),
  );

  ipcMain.handle(
    "window_toggle_maximize",
    withRuntime(async () => {
      const window = getTargetWindow();
      if (!window) {
        return;
      }

      if (window.isMaximized()) {
        window.unmaximize();
        return;
      }

      window.maximize();
    }),
  );

  ipcMain.handle(
    "window_close",
    withRuntime(async () => {
      getTargetWindow()?.close();
    }),
  );

  ipcMain.handle(
    "shell_open_path",
    withRuntime(
      async (_runtime, payload: { path: string }) =>
        await shell.openPath(payload.path),
    ),
  );

  ipcMain.handle(
    "shell_open_external",
    withRuntime(async (_runtime, payload: { url: string }) => {
      await shell.openExternal(payload.url);
    }),
  );

  ipcMain.handle(
    "get_autostart_state",
    withRuntime(async () => app.getLoginItemSettings().openAtLogin),
  );

  ipcMain.handle(
    "set_autostart_state",
    withRuntime(async (_runtime, payload: { enabled: boolean }) => {
      app.setLoginItemSettings({
        openAtLogin: payload.enabled,
        path: process.execPath,
      });
    }),
  );

  ipcMain.handle(
    "cancel_scan",
    withRuntime(async (runtime) => {
      runtime.currentScan?.cancel();
      runtime.currentScan = null;
    }),
  );

  ipcMain.handle(
    "scan_executables_stream",
    withRuntime(async (runtime, payload: { dir: string }) => {
      runtime.currentScan?.cancel();
      const cancellation = createScanCancellation();
      runtime.currentScan = cancellation;
      let count = 0;

      try {
        count = await scanExecutablesStream(payload.dir, {
          signal: cancellation.signal,
          onEntry: async (entry) => {
            deps.emitRendererEvent("scan:entry", entry);
          },
        });
        await runtime.services.achievements.recordAchievementEvent("scan_complete");
        return count;
      } finally {
        if (runtime.currentScan === cancellation) {
          runtime.currentScan = null;
        }
        deps.emitRendererEvent("scan:done", { count });
      }
    }),
  );
}
