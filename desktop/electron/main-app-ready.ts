import { nativeTheme, powerMonitor } from "electron";
import type { ArkClient } from "@kosmos/ark";
import type { JsonRecord } from "./extension-permissions";
import { registerMainProtocols } from "./main-protocols";
import { handlePostUpdateFirstLaunch } from "./main-post-update";
import { setupAutoUpdater } from "./autoupdater-host";
import {
  getStoredHotkey,
  isTrayIconEnabled,
  normalizeHotkeyAccelerator,
  setHotkeyReregisterCallback,
  setTrayVisibilityController,
} from "./settings-window";

interface AppReadyBackendSupervisor {
  initArkClient(): Promise<void>;
  markBootInitStarted(): void;
  recoverBackendIfDead(reason: string): Promise<void>;
  spawnBackend(): void;
  awaitArkReady(): Promise<ArkClient>;
}

interface AppReadyLogger {
  error(scope: string, message: string, data?: JsonRecord): void;
}

interface AppReadyInstance {
  autoupdaterEnabled: boolean;
  hotkey: string | null;
  slot: string;
}

type PostUpdateLauncher = Parameters<typeof handlePostUpdateFirstLaunch>[0];

interface AppReadyLauncher extends PostUpdateLauncher {
  createLauncher(): void;
  showLauncher(): void;
  openManager(): void;
  registerLauncherHotkeys(options: {
    slot: string;
    slotHotkey: string | null;
    getStoredHotkey: typeof getStoredHotkey;
    normalizeHotkeyAccelerator: typeof normalizeHotkeyAccelerator;
    setHotkeyReregisterCallback: typeof setHotkeyReregisterCallback;
  }): void;
  setTrayVisible(visible: boolean): void;
}

interface RunAppReadyOptions {
  awaitArkReady: Parameters<typeof registerMainProtocols>[0]["awaitArkReady"];
  backendSupervisor: AppReadyBackendSupervisor;
  instance: AppReadyInstance;
  launcher: AppReadyLauncher;
  log: AppReadyLogger;
  runBootSelfCheck(): void;
  recoverLegacyMigration?(): Promise<void>;
  runLegacyMigration?(): Promise<void>;
}

export async function runAppReady({
  awaitArkReady,
  backendSupervisor,
  instance,
  launcher,
  log,
  runBootSelfCheck,
  recoverLegacyMigration,
  runLegacyMigration,
}: RunAppReadyOptions): Promise<void> {
  runBootSelfCheck();
  nativeTheme.themeSource = "dark";

  if (process.argv.includes("--autostart")) {
    console.log(
      "[kepler-shell] launched from Windows autorun (--autostart marker present); launcher remains hidden, tray icon only",
    );
  }

  backendSupervisor.spawnBackend();
  registerMainProtocols({ awaitArkReady });
  backendSupervisor.markBootInitStarted();
  const boot = backendSupervisor.initArkClient();
  if (!process.argv.includes("--autostart")) {
    const openManager =
      process.env.KOSMOS_TEST_MODE === "1" ? launcher.showLauncher : launcher.openManager;
    openManager();
  }
  void boot.catch((error) => {
    log.error(
      "startup",
      "Ark client initialization failed",
      error instanceof Error && error.stack
        ? { err: String(error), stack: error.stack }
        : { err: String(error) },
    );
  });
  setTrayVisibilityController(launcher.setTrayVisible);
  launcher.setTrayVisible(isTrayIconEnabled());

  powerMonitor.on("resume", () => {
    void backendSupervisor.recoverBackendIfDead("power-resume").catch((error) => {
      log.error(
        "supervisor",
        "power-resume recovery failed",
        error instanceof Error && error.stack
          ? { reason: "power-resume", err: String(error), stack: error.stack }
          : { reason: "power-resume", err: String(error) },
      );
    });
  });

  setupAutoUpdater({ isDev: !instance.autoupdaterEnabled });
  handlePostUpdateFirstLaunch(launcher);

  launcher.registerLauncherHotkeys({
    slot: instance.slot,
    slotHotkey: instance.hotkey,
    getStoredHotkey,
    normalizeHotkeyAccelerator,
    setHotkeyReregisterCallback,
  });

  if (runLegacyMigration) {
    void (async () => {
      try {
        await backendSupervisor.awaitArkReady();
        if (recoverLegacyMigration) await recoverLegacyMigration();
        await runLegacyMigration();
      } catch (error) {
        log.error(
          "migration",
          "legacy migration failed",
          error instanceof Error && error.stack
            ? { err: String(error), stack: error.stack }
            : { err: String(error) },
        );
      }
    })();
  }
}
