import { app, BrowserWindow, crashReporter, dialog, globalShortcut, protocol } from "electron";
import { resolveInstance, applyInstanceToApp, verifyUserDataMatches } from "./instance";

// КРИТИЧНО: applyInstanceToApp ДОЛЖЕН выполниться до requestSingleInstanceLock
// и до любого app.getPath('userData') / app.getName() — Electron кэширует эти
// значения и singleInstanceLock scope'ится по userData. Импорты ниже могут
// потянуть модули, которые читают app.getPath('userData') на module top-level
// (settings-window.ts, autoupdater-host.ts), — поэтому здесь, не в bootstrap().
const KEPLER_INSTANCE = resolveInstance();
applyInstanceToApp(KEPLER_INSTANCE);

// macOS: overlay-окна (dictation pill, focus widget, block overlay) — frameless
// полупрозрачные окна без постоянного always-on-top.
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
  safeHandle,
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
import { openManager } from "./manager-navigation";
import { check as checkUpdates, install as installUpdate } from "./autoupdater-host";
import { registerMainProcessIpc } from "./main-ipc-registrations";
import {
  resolveBackendExePath,
  BACKEND_TRAY_EXIT_CODE,
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
const env = process.env;

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
  onBackendExit: (code) => code === BACKEND_TRAY_EXIT_CODE && !isQuiting && app.quit(),
});

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
  openManager();
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

void app
  .whenReady()
  .then(() =>
    runAppReady({
      awaitArkReady,
      backendSupervisor,
      instance: KEPLER_INSTANCE,
      log: keplerLog,
      runBootSelfCheck,
      recoverLegacyMigration: async () => {
        const client = await backendSupervisor.awaitArkReady();
        await recoverLegacyMigrationsBeforeLaunch(KEPLER_INSTANCE.dataDir, client);
      },
      runLegacyMigration: async () => {
        const client = await backendSupervisor.awaitArkReady();
        await createLegacyMigrationRunner(KEPLER_INSTANCE.dataDir, client).run();
      },
    }),
  )
  .catch((error) => {
    keplerLog.error(
      "startup",
      "app ready failed",
      error instanceof Error && error.stack
        ? { err: String(error), stack: error.stack }
        : { err: String(error) },
    );
  });

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
