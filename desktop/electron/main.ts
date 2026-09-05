// Kepler — Electron host для Kosmos ecosystem.
//
// Phase 1 baseline (scaffold):
//   1. Spawn kepler-backend.exe child (singleton, держит ARK).
//   2. Один frameless transparent BrowserWindow по центру (launcher).
//   3. Окно hidden default; globalShortcut Ctrl+Shift+K (Win/Linux) /
//      Cmd+Shift+K (macOS) toggle show/hide.
//   4. backgroundMaterial: 'mica' (Win11) — graceful fallback на flat на
//      older Windows / non-Win платформах.
//   5. app.requestSingleInstanceLock — одна копия Kepler на машину.
//   6. Tray icon с menu Open/Quit.
//
// Phase 2+ задачи (не в этом scaffold'е):
//   - Полноценное extension API в preload (window.kepler.extensions.*).
//   - WS-client к kepler-backend (FTS5 search, quick-create).
//   - Window state persistence.
//   - Auto-update mechanism.

import { app, BrowserWindow, crashReporter, dialog, globalShortcut, protocol } from "electron";
import { resolveInstance, applyInstanceToApp, verifyUserDataMatches } from "./instance";

// КРИТИЧНО: applyInstanceToApp ДОЛЖЕН выполниться до requestSingleInstanceLock
// и до любого app.getPath('userData') / app.getName() — Electron кэширует эти
// значения и singleInstanceLock scope'ится по userData. Импорты ниже могут
// потянуть модули, которые читают app.getPath('userData') на module top-level
// (settings-window.ts, autoupdater-host.ts), — поэтому здесь, не в bootstrap().
const KEPLER_INSTANCE = resolveInstance();
applyInstanceToApp(KEPLER_INSTANCE);

// macOS: launcher — frameless полупрозрачное окно без постоянного always-on-top.
// macOS Window Server помечает такое окно как occluded, и Chromium останавливает
// compositor (paint замерзает через 1-2с после показа — Vue реактивность жива,
// но экран не перерисовывается). Эти switch'и отключают occlusion-throttling на
// уровне Chromium. Должны быть выставлены ДО app.whenReady. См. postmortems.md.
if (process.platform === "darwin") {
  app.commandLine.appendSwitch("disable-backgrounding-occluded-windows");
  app.commandLine.appendSwitch("disable-renderer-backgrounding");
}
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  APP_ICON_PROTOCOL,
  LOCAL_IMAGE_PROTOCOL,
  keplerLog,
  resolveWindowMaterial,
  safeHandle,
  type KosmosWindowMaterial,
} from "./main-shell-services";
import {
  setupFocusWidgetBackendSync,
  teardownFocusWidgetBackendSync,
  setupMainDictationRuntime,
  setupMainFocusRuntime,
  setupFocusSessionBackendSync,
  setupDictationHotkey,
  setupPomodoroNotifier,
  teardownFocusSessionBackendSync,
  teardownPomodoroNotifier,
} from "./main-runtime-integrations";
import { createLauncherController } from "./main-launcher";
import { openSettings } from "./settings-window";
import { check as checkUpdates, install as installUpdate } from "./autoupdater-host";
import { registerMainProcessIpc } from "./main-ipc-registrations";
import {
  resolveBackendExePath,
  runBootSelfCheckWithDeps,
  createMainBackendSupervisor,
  runAppReady,
} from "./main-backend-entry";
import {
  createLegacyMigrationRunner,
  recoverLegacyMigrationsBeforeLaunch,
} from "./legacy-migration-runtime";

// См. postmortems.md § 2026-05-30: focus-block dynamic chunk imports from
// main.js after Vite/Rolldown code-splitting, so these helper APIs must remain
// visible on the entry module namespace.

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const WINDOW_WIDTH = 720;
const WINDOW_HEIGHT = 460;
const env = process.env;
const devServerUrl = env.VITE_DEV_SERVER_URL;

function resolveLauncherBgMaterial(): KosmosWindowMaterial {
  return resolveWindowMaterial("none");
}

protocol.registerSchemesAsPrivileged([
  {
    scheme: APP_ICON_PROTOCOL,
    privileges: {
      standard: true,
      secure: true,
      supportFetchAPI: true,
    },
  },
  {
    scheme: LOCAL_IMAGE_PROTOCOL,
    privileges: {
      standard: true,
      secure: true,
      supportFetchAPI: true,
      corsEnabled: true,
    },
  },
]);

let isQuiting = false;
let cleanupStarted = false;

const backendSupervisor = createMainBackendSupervisor({
  env,
  instance: KEPLER_INSTANCE,
  resolveBackendExe,
  getIsQuiting: () => isQuiting,
  setupPomodoroNotifier,
  teardownPomodoroNotifier,
  setupFocusWidgetBackendSync,
  teardownFocusWidgetBackendSync,
  setupFocusSessionBackendSync,
  teardownFocusSessionBackendSync,
  setupDictationHotkey,
  broadcastCommandsUpdated,
});

const launcherController = createLauncherController({
  isDev: !!devServerUrl,
  productName: KEPLER_INSTANCE.productName,
  windowWidth: WINDOW_WIDTH,
  windowHeight: WINDOW_HEIGHT,
  dirname: __dirname,
  devServerUrl,
  resolveBackgroundMaterial: resolveLauncherBgMaterial,
  getIsQuiting: () => isQuiting,
  openSettings,
  quitApplication: () => {
    isQuiting = true;
    app.quit();
  },
  onLauncherShow: () => void backendSupervisor.recoverBackendIfDead("launcher-show"),
});

const { hideLauncher, setLauncherExpanded } = launcherController;

export function broadcastCommandsUpdated(): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      try {
        win.webContents.send("kepler:commands:updated");
      } catch {
        /* ignore */
      }
    }
  }
}

registerMainProcessIpc({
  awaitArkReady,
  getArkClient: () => backendSupervisor.getArkClient(),
  hideLauncher,
  setLauncherExpanded,
});

setupMainFocusRuntime(awaitArkReady);
setupMainDictationRuntime({
  awaitArkReady,
  broadcastCommandsUpdated,
  getArkClient: () => backendSupervisor.getArkClient(),
});

export function awaitArkReady(timeoutMs?: number) {
  return backendSupervisor.awaitArkReady(timeoutMs);
}
// --- single instance ---------------------------------------------------------

if (!app.requestSingleInstanceLock()) {
  app.quit();
  process.exit(0);
}

app.on("second-instance", (_event, argv) => {
  if (argv.includes("--autostart")) return;
  if (argv.includes("--kosmos-update-check")) {
    void checkUpdates();
    return;
  }
  if (argv.includes("--kosmos-update-install")) {
    installUpdate();
    return;
  }
  launcherController.openManager();
});

// --- Electron crash reporter ------------------------------------------------
// Должен быть инициализирован ДО app.whenReady. uploadToServer: false —
// minidump'ы остаются локально в app.getPath("crashDumps") (по умолчанию
// %APPDATA%/<productName>/Crashpad/). Submit вручную из Settings UI.
crashReporter.start({
  // productName выбирает поддиректорию для minidump'ов:
  //   %APPDATA%/<productName>/Crashpad/. Per-slot — не смешиваем dev и prod
  //   crash'и.
  productName: KEPLER_INSTANCE.productName,
  companyName: "Kosmos",
  uploadToServer: false,
  submitURL: "",
  compress: false,
});

app.on("render-process-gone", (_event, webContents, details) => {
  keplerLog.crash("electron-renderer", {
    webContentsId: webContents.id,
    reason: details.reason,
    exitCode: details.exitCode,
  });
});

app.on("child-process-gone", (_event, details) => {
  keplerLog.crash(`electron-${details.type}`, {
    reason: details.reason,
    exitCode: details.exitCode,
    serviceName: details.serviceName ?? null,
    name: details.name ?? null,
  });
});

// --- backend spawn -----------------------------------------------------------

function resolveBackendExe(): string {
  return resolveBackendExePath({
    dirname: __dirname,
    env,
    resourcesPath: process.resourcesPath ?? __dirname,
  });
}

function runBootSelfCheck(): void {
  runBootSelfCheckWithDeps({
    backendExe: resolveBackendExe(),
    env,
    exit: (code) => app.exit(code),
    instance: KEPLER_INSTANCE,
    log: keplerLog,
    showErrorBox: (title, content) => dialog.showErrorBox(title, content),
    verifyUserDataMatches,
  });
}

safeHandle("kepler:backend:restart", async () => {
  await backendSupervisor.restartBackend();
});

// --- lifecycle ---------------------------------------------------------------

void app.whenReady().then(() =>
  runAppReady({
    awaitArkReady,
    backendSupervisor,
    instance: KEPLER_INSTANCE,
    launcher: launcherController,
    runBootSelfCheck,
    recoverLegacyMigration: () => recoverLegacyMigrationsBeforeLaunch(KEPLER_INSTANCE.dataDir),
    runLegacyMigration: async () => {
      const client = await backendSupervisor.awaitArkReady();
      await createLegacyMigrationRunner(KEPLER_INSTANCE.dataDir, client).run();
    },
  }),
);

app.on("window-all-closed", () => {
  app.quit();
});

// Любой источник app.quit() (Playwright, programmatic, signals) должен
// взвести isQuiting — иначе mainWindow.close handler делает preventDefault
// (тarayresident-режим) и quit блокируется.
app.on("before-quit", () => {
  isQuiting = true;
});

app.on("will-quit", (event) => {
  event.preventDefault();
  if (cleanupStarted) return;
  cleanupStarted = true;
  console.error("[kepler-shell] will-quit: starting cleanup");
  globalShortcut.unregisterAll();
  void backendSupervisor.shutdown().finally(() => {
    console.error("[kepler-shell] will-quit: cleanup done");
    app.exit(0);
  });
});
