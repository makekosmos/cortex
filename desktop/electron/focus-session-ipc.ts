import { ipcMain } from "electron";

import type {
  DelphiTask,
  FocusBlocklist,
  FocusSessionSnapshot,
  StartFocusSessionInput,
} from "./focus-session-types";

export function registerFocusSessionIpc(handlers: {
  open: () => void;
  snapshot: () => Promise<FocusSessionSnapshot>;
  listTasks: () => Promise<DelphiTask[]>;
  listBlocklists: () => Promise<FocusBlocklist[]>;
  start: (input: StartFocusSessionInput) => Promise<FocusSessionSnapshot>;
  pause: () => Promise<FocusSessionSnapshot>;
  resume: () => Promise<FocusSessionSnapshot>;
  skip: () => Promise<FocusSessionSnapshot>;
  stop: () => Promise<FocusSessionSnapshot>;
  complete: () => Promise<FocusSessionSnapshot>;
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
