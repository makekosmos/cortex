import { nativeTheme, powerMonitor } from "electron";
import type { ArkClient } from "@kosmos/ark";
import { showFocusBlockOverlay } from "./focus-overlay";
import {
  setBlockedAppNotifier,
  setFocusSessionRuntime,
  setFocusSessionShellOpener,
} from "./focus-session";
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
  showFocusSessionLauncher(): void;
}

interface RunAppReadyOptions {
  awaitArkReady: Parameters<typeof registerMainProtocols>[0]["awaitArkReady"];
  backendSupervisor: AppReadyBackendSupervisor;
  instance: AppReadyInstance;
  launcher: AppReadyLauncher;
  runBootSelfCheck(): void;
  recoverLegacyMigration?(): Promise<void>;
  runLegacyMigration?(): Promise<void>;
}

export async function runAppReady({
  awaitArkReady,
  backendSupervisor,
  instance,
  launcher,
  runBootSelfCheck,
  recoverLegacyMigration,
  runLegacyMigration,
}: RunAppReadyOptions): Promise<void> {
  runBootSelfCheck();
  if (recoverLegacyMigration) await recoverLegacyMigration();
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
  if (runLegacyMigration) {
    try {
      await backendSupervisor.awaitArkReady();
      await runLegacyMigration();
    } catch (error) {
      console.error("[kepler-shell] legacy migration deferred:", error);
    }
  }
  if (!process.argv.includes("--autostart")) {
    const openManager =
      process.env.KOSMOS_TEST_MODE === "1" ? launcher.showLauncher : launcher.openManager;
    openManager();
    void boot;
  } else void boot;
  setFocusSessionShellOpener(launcher.showFocusSessionLauncher);
  setFocusSessionRuntime({ awaitArkReady });
  setBlockedAppNotifier((app) => {
    showFocusBlockOverlay(app);
  });
  setTrayVisibilityController(launcher.setTrayVisible);
  launcher.setTrayVisible(isTrayIconEnabled());

  powerMonitor.on("resume", () => {
    void backendSupervisor.recoverBackendIfDead("power-resume");
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
}
