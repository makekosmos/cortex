// Phase 7 bug-detection: safe IPC handler wrapper.
//
// Что: оборачивает ipcMain.handle. При throw'е в handler'е логирует через
// keplerLog.error с channel name + stack и re-throw'ит (renderer получит
// rejection как обычно — мы НЕ swallow'им ошибки, иначе UI отобразит
// «successful» при failure).
//
// Зачем: до этого каждый author IPC handler'а должен был руками писать
// try/catch + console.error. На практике ~половина handler'ов либо без
// catch'а вообще (throw улетает в стек Electron'а и теряется), либо с
// `console.error` (Phase 4 уже зафиксировала: stderr пропадает после exit).
// safeHandle гарантирует structured log для каждого failure без boilerplate.
//
// Use:
//   safeHandle("kepler:foo", async (_e, x: number) => {
//     return something(x);
//   });

import { ipcMain } from "electron";
import { keplerLog } from "./logging";

export function safeHandle<P extends unknown[], R>(
  channel: string,
  handler: (event: Electron.IpcMainInvokeEvent, ...args: P) => Promise<R> | R,
): void {
  ipcMain.handle(channel, (event, ...args) =>
    Promise.resolve()
      .then(() => handler(event, ...(args as P)))
      .catch((err) => {
        keplerLog.error("ipc", `${channel} threw`, {
          err: String(err),
          stack: err instanceof Error ? err.stack : undefined,
        });
        throw err;
      }),
  );
}
