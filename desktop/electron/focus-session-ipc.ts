import { ipcMain } from "electron";

import type { StartFocusSessionInput } from "./focus-session-types";

export function registerFocusSessionIpc(handlers: {
  open: () => void;
  snapshot: () => Promise<unknown>;
  listTasks: () => Promise<unknown>;
  listBlocklists: () => Promise<unknown>;
  start: (input: StartFocusSessionInput) => Promise<unknown>;
  pause: () => Promise<unknown>;
  resume: () => Promise<unknown>;
  skip: () => Promise<unknown>;
  stop: () => Promise<unknown>;
  complete: () => Promise<unknown>;
  snoozeApp: (appId: string) => void;
}): void {
  ipcMain.handle("kepler:focus-session:open", () => {
    handlers.open();
  });
  ipcMain.handle("kepler:focus-session:snapshot", () => handlers.snapshot());
  ipcMain.handle("kepler:focus-session:list-tasks", () => handlers.listTasks());
  ipcMain.handle("kepler:focus-session:list-blocklists", () => handlers.listBlocklists());
  ipcMain.handle("kepler:focus-session:start", (_e, input: StartFocusSessionInput) =>
    handlers.start(input),
  );
  ipcMain.handle("kepler:focus-session:pause", () => handlers.pause());
  ipcMain.handle("kepler:focus-session:resume", () => handlers.resume());
  ipcMain.handle("kepler:focus-session:skip", () => handlers.skip());
  ipcMain.handle("kepler:focus-session:stop", () => handlers.stop());
  ipcMain.handle("kepler:focus-session:complete", () => handlers.complete());
  ipcMain.handle("kepler:focus-session:snooze-app", (_e, appId: string) =>
    handlers.snoozeApp(appId),
  );
}
