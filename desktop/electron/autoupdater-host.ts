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

import { BrowserWindow, dialog } from "electron";
import electronUpdater from "electron-updater";
import { writeFileSync } from "node:fs";
import path from "node:path";
import { keplerDataDir } from "./data-dir";
import { readEmbeddedReleaseBomIdentity } from "./release-bom-identity";
import type { ReleaseBomIdentity, UpdateState as SharedUpdateState } from "../shared/ipc-types";

export type UpdateState = SharedUpdateState;

const STATE_CHANGED_CHANNEL = "kepler:settings:update:state";

const embeddedBom = readEmbeddedReleaseBomIdentity();
let currentState: UpdateState = embeddedBom ? { kind: "idle", bom: embeddedBom } : { kind: "idle" };
let initialized = false;
const stateFile = () => path.join(keplerDataDir(), "update-state.json");

function bomState(): { bom?: ReleaseBomIdentity } {
  return embeddedBom ? { bom: embeddedBom } : {};
}

function broadcast(state: UpdateState): void {
  currentState = state;
  try {
    writeFileSync(stateFile(), JSON.stringify(state), "utf8");
  } catch (e) {
    console.warn("[autoUpdater] failed to persist state:", e);
  }
  for (const win of BrowserWindow.getAllWindows()) {
    if (win.isDestroyed()) continue;
    try {
      win.webContents.send(STATE_CHANGED_CHANNEL, state);
    } catch {
      /* dead webContents — skip */
    }
  }
}

function scheduleNativeInstallFallback(version: string): void {
  setTimeout(
    () => {
      if (currentState.kind !== "downloaded") return;
      void dialog
        .showMessageBox({
          type: "info",
          title: "Обновление Kosmos готово",
          message: `Версия ${version} скачана. Перезапустить сейчас?`,
          buttons: ["Перезапустить", "Позже"],
          defaultId: 0,
          cancelId: 1,
        })
        .then((result) => {
          if (result.response === 0) electronUpdater.autoUpdater.quitAndInstall();
        });
    },
    5 * 60 * 1000,
  );
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
    broadcast({ kind: "checking", ...bomState() });
  });
  autoUpdater.on("update-available", (info) => {
    console.error("[autoUpdater] available:", info.version);
    broadcast({ kind: "available", version: info.version, ...bomState() });
  });
  autoUpdater.on("update-not-available", () => {
    broadcast({ kind: "not-available", checkedAt: Date.now(), ...bomState() });
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
      ...bomState(),
    });
  });
  autoUpdater.on("update-downloaded", (info) => {
    console.error("[autoUpdater] downloaded:", info.version);
    broadcast({ kind: "downloaded", version: info.version, ...bomState() });
    // Не показываем native dialog — UI banner драйвит click-to-install.
    // Native fallback оставляем для случая когда Settings window закрыт >5 min.
    scheduleNativeInstallFallback(info.version);
  });
  autoUpdater.on("error", (err) => {
    console.error("[autoUpdater] error:", err);
    broadcast({ kind: "error", message: err.message, ...bomState() });
  });

  void check();
  setInterval(() => void check(), 6 * 60 * 60 * 1000);
}

export async function check(): Promise<UpdateState> {
  if (!initialized) return currentState;
  try {
    await electronUpdater.autoUpdater.checkForUpdates();
  } catch (e) {
    // SAFETY: The surrounding boundary establishes this documented contract.
    broadcast({ kind: "error", message: (e as Error).message, ...bomState() });
  }
  return currentState;
}

export function install(): void {
  if (!initialized) return;
  // Флаг для post-update first-launch: при следующем старте main.ts его
  // прочитает и покажет в launcher'е баннер «Kepler обновлён». После чтения
  // флаг очищается.
  //
  // ВАЖНО: внутри install() `app.getVersion()` возвращает СТАРУЮ версию
  // (процесс ещё не перезапущен с новым кодом). Пишем только timestamp;
  // новую версию main.ts при чтении флага возьмёт из `app.getVersion()`
  // ПОСЛЕ старта — это уже новая.
  try {
    const flag = path.join(keplerDataDir(), "post-update.flag");
    writeFileSync(flag, JSON.stringify({ at: Date.now() }), "utf8");
  } catch (e) {
    console.warn("[autoUpdater] failed to write post-update flag:", e);
  }
  electronUpdater.autoUpdater.quitAndInstall();
}

export function getState(): UpdateState {
  return currentState;
}
