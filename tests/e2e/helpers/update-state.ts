// Helpers для emit'а autoupdater state и post-update event'а из main process
// в renderer без участия настоящего electron-updater (он skip'ается в test mode,
// см. autoupdater-host.ts → `KOSMOS_TEST_MODE === "1"` guard).
//
// Используется в launcher-update-progress.spec.ts и launcher-post-update.spec.ts
// (а также в любых будущих spec'ах update flow — fake state через webContents.send).
//
// Wire-формат должен совпадать с реальным:
//   - "kepler:settings:update:state" payload — `UpdateState` discriminated union
//     (см. autoupdater-host.ts).
//   - "kepler:post-update" payload — `{ version: string }`
//     (см. main.ts post-update flow).

import type { ElectronApplication } from "playwright";

export type FakeUpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "not-available"; checkedAt: number }
  | { kind: "available"; version: string }
  | { kind: "downloading"; version: string; percent: number }
  | { kind: "downloaded"; version: string }
  | { kind: "error"; message: string };

/**
 * Шлёт fake UpdateState на все живые webContents (как broadcast в
 * autoupdater-host.ts → broadcast()).
 */
export async function emitUpdateState(
  app: ElectronApplication,
  state: FakeUpdateState,
): Promise<void> {
  await app.evaluate(({ BrowserWindow }, payload) => {
    for (const win of BrowserWindow.getAllWindows()) {
      if (win.isDestroyed()) continue;
      try {
        win.webContents.send("kepler:settings:update:state", payload);
      } catch {
        /* dead webContents */
      }
    }
  }, state);
}

/**
 * Шлёт fake post-update событие (как main.ts post-update flow → webContents.send).
 * НЕ показывает launcher автоматически — для тестов «launcher уже видим, баннер появляется».
 * Для интеграционного теста с launch'ем — записывай real flag-файл, см. writePostUpdateFlag.
 */
export async function emitPostUpdate(
  app: ElectronApplication,
  payload: { version: string },
): Promise<void> {
  await app.evaluate(({ BrowserWindow }, p) => {
    for (const win of BrowserWindow.getAllWindows()) {
      if (win.isDestroyed()) continue;
      try {
        win.webContents.send("kepler:post-update", p);
      } catch {
        /* dead webContents */
      }
    }
  }, payload);
}

/**
 * Возвращает реальный `app.getPath("userData")` после applyInstanceToApp —
 * для test slot'а это `<KOSMOS_DATA_DIR>/userdata`, а не `--user-data-dir=`.
 */
export async function getUserDataDir(app: ElectronApplication): Promise<string> {
  return await app.evaluate(({ app: a }) => a.getPath("userData"));
}
