// autoUpdater host: state machine over electron-updater event'ов.
//
// Поверх API electron-updater добавляет:
//   1. Broadcast UpdateState к Settings window (and any subscribers через IPC),
//      чтобы UI мог отрисовать persistent banner Raycast-style.
//   2. Manual `check()` для кнопки «Проверить обновления».
//   3. `install()` (quitAndInstall) — клик по banner'у в готовом состоянии.
//
// State machine:
//   idle → checking → (available → downloading → downloaded) | not-available | error
//   any → checking (manual or periodic)

import { app, BrowserWindow, dialog } from "electron";
import electronUpdater from "electron-updater";
import { writeFileSync } from "node:fs";
import path from "node:path";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "not-available"; checkedAt: number }
  | { kind: "available"; version: string }
  | { kind: "downloading"; version: string; percent: number }
  | { kind: "downloaded"; version: string }
  | { kind: "error"; message: string };

const STATE_CHANGED_CHANNEL = "kepler:settings:update:state";

let currentState: UpdateState = { kind: "idle" };
let initialized = false;

function broadcast(state: UpdateState): void {
  currentState = state;
  for (const win of BrowserWindow.getAllWindows()) {
    if (win.isDestroyed()) continue;
    try {
      win.webContents.send(STATE_CHANGED_CHANNEL, state);
    } catch {
      /* dead webContents — skip */
    }
  }
}

export function setupAutoUpdater(opts: { isDev: boolean }): void {
  if (initialized) return;
  if (opts.isDev) {
    console.error("[autoUpdater] skipped в dev mode");
    return;
  }
  if (process.env.KOSMOS_TEST_MODE === "1") {
    console.error("[autoUpdater] skipped в test mode");
    return;
  }
  initialized = true;

  const { autoUpdater } = electronUpdater;
  autoUpdater.logger = console;
  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = false;

  autoUpdater.on("checking-for-update", () => {
    broadcast({ kind: "checking" });
  });
  autoUpdater.on("update-available", (info) => {
    console.error("[autoUpdater] available:", info.version);
    broadcast({ kind: "available", version: info.version });
  });
  autoUpdater.on("update-not-available", () => {
    broadcast({ kind: "not-available", checkedAt: Date.now() });
  });
  autoUpdater.on("download-progress", (p) => {
    const version =
      currentState.kind === "downloading" || currentState.kind === "available"
        ? currentState.version
        : "";
    broadcast({
      kind: "downloading",
      version,
      percent: Math.round(p.percent),
    });
  });
  autoUpdater.on("update-downloaded", (info) => {
    console.error("[autoUpdater] downloaded:", info.version);
    broadcast({ kind: "downloaded", version: info.version });
    // Не показываем native dialog — UI banner драйвит click-to-install.
    // Native fallback оставляем для случая когда Settings window закрыт >5 min.
    setTimeout(() => {
      if (currentState.kind !== "downloaded") return;
      void dialog
        .showMessageBox({
          type: "info",
          title: "Kepler обновление готово",
          message: `Версия ${info.version} скачана. Перезапустить сейчас?`,
          buttons: ["Перезапустить", "Позже"],
          defaultId: 0,
          cancelId: 1,
        })
        .then((result) => {
          if (result.response === 0) autoUpdater.quitAndInstall();
        });
    }, 5 * 60 * 1000);
  });
  autoUpdater.on("error", (err) => {
    console.error("[autoUpdater] error:", err);
    broadcast({ kind: "error", message: err.message });
  });

  void check();
  setInterval(() => void check(), 6 * 60 * 60 * 1000);
}

export async function check(): Promise<UpdateState> {
  if (!initialized) return currentState;
  try {
    await electronUpdater.autoUpdater.checkForUpdates();
  } catch (e) {
    broadcast({ kind: "error", message: (e as Error).message });
  }
  return currentState;
}

export function install(): void {
  if (!initialized) return;
  // Флаг для post-update first-launch: при следующем старте main.ts его
  // прочитает, откроет launcher и покажет changelog модалку. После чтения
  // флаг очищается.
  try {
    const flag = path.join(app.getPath("userData"), "post-update.flag");
    writeFileSync(flag, String(Date.now()), "utf8");
  } catch (e) {
    console.warn("[autoUpdater] failed to write post-update flag:", e);
  }
  electronUpdater.autoUpdater.quitAndInstall();
}

export function getState(): UpdateState {
  return currentState;
}
