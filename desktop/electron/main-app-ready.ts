import { nativeTheme, powerMonitor } from "electron";
import { CLIPBOARD_HISTORY_ENABLED } from "../shared/ipc-types";
import {
  registerClipboardHistoryIpc,
  setClipboardHistoryShellOpener,
  startClipboardHistory,
} from "./clipboard-history";
import { setExtensionArkBridgeReadyTimeoutMs } from "./extension-host";
import { registerMarketplaceIpc, startPeriodicCatalogCheck } from "./extension-marketplace";
import { showFocusBlockOverlay } from "./focus-overlay";
import {
  setBlockedAppNotifier,
  setFocusSessionRuntime,
  setFocusSessionShellOpener,
} from "./focus-session";
import { registerMainProtocols } from "./main-protocols";
import { handlePostUpdateFirstLaunch } from "./main-post-update";
import { scheduleBenchmarkOpenAllExtensions } from "./main-benchmark-open-all";
import { openInitialKextFromArgv } from "./main-initial-kext";
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
}

interface AppReadyInstance {
  autoupdaterEnabled: boolean;
  hotkey: string | null;
  periodicMarketplaceCheckEnabled: boolean;
  slot: string;
}

type PostUpdateLauncher = Parameters<typeof handlePostUpdateFirstLaunch>[0];

interface AppReadyLauncher extends PostUpdateLauncher {
  createLauncher(): void;
  registerLauncherHotkeys(options: {
    slot: string;
    slotHotkey: string | null;
    getStoredHotkey: typeof getStoredHotkey;
    normalizeHotkeyAccelerator: typeof normalizeHotkeyAccelerator;
    setHotkeyReregisterCallback: typeof setHotkeyReregisterCallback;
  }): void;
  setTrayVisible(visible: boolean): void;
  showClipboardHistoryLauncher(): void;
  showFocusSessionLauncher(): void;
}

interface RunAppReadyOptions {
  arkReadyRequestTimeoutMs: number;
  awaitArkReady: Parameters<typeof registerMainProtocols>[0]["awaitArkReady"];
  backendSupervisor: AppReadyBackendSupervisor;
  env: NodeJS.ProcessEnv;
  instance: AppReadyInstance;
  launcher: AppReadyLauncher;
  runBootSelfCheck(): void;
}

export function shouldShowLauncherOnStartup(argv: readonly string[]): boolean {
  return !argv.includes("--autostart");
}

export async function runAppReady({
  arkReadyRequestTimeoutMs,
  awaitArkReady,
  backendSupervisor,
  env,
  instance,
  launcher,
  runBootSelfCheck,
}: RunAppReadyOptions): Promise<void> {
  runBootSelfCheck();
  nativeTheme.themeSource = "dark";

  if (process.argv.includes("--autostart")) {
    console.log(
      "[kepler-shell] launched from Windows autorun (--autostart marker present); launcher remains hidden, tray icon only",
    );
  }

  backendSupervisor.spawnBackend();
  setExtensionArkBridgeReadyTimeoutMs(arkReadyRequestTimeoutMs);
  registerMainProtocols({ awaitArkReady });
  launcher.createLauncher();
  if (CLIPBOARD_HISTORY_ENABLED) {
    setClipboardHistoryShellOpener(launcher.showClipboardHistoryLauncher);
  }
  setFocusSessionShellOpener(launcher.showFocusSessionLauncher);
  setFocusSessionRuntime({ awaitArkReady });
  setBlockedAppNotifier((app) => {
    showFocusBlockOverlay(app);
  });
  setTrayVisibilityController(launcher.setTrayVisible);
  launcher.setTrayVisible(isTrayIconEnabled());
  if (shouldShowLauncherOnStartup(process.argv)) {
    launcher.showLauncher();
  }

  backendSupervisor.markBootInitStarted();
  void backendSupervisor.initArkClient();
  powerMonitor.on("resume", () => {
    void backendSupervisor.recoverBackendIfDead("power-resume");
  });

  if (CLIPBOARD_HISTORY_ENABLED) {
    registerClipboardHistoryIpc();
    startClipboardHistory();
  }
  registerMarketplaceIpc();
  setupAutoUpdater({ isDev: !instance.autoupdaterEnabled });
  handlePostUpdateFirstLaunch(launcher);

  if (env.KOSMOS_TEST_MODE !== "1" && instance.periodicMarketplaceCheckEnabled) {
    startPeriodicCatalogCheck();
  }

  openInitialKextFromArgv(process.argv);

  if (env.KEPLER_BENCHMARK_OPEN_ALL === "1") {
    scheduleBenchmarkOpenAllExtensions();
  }

  launcher.registerLauncherHotkeys({
    slot: instance.slot,
    slotHotkey: instance.hotkey,
    getStoredHotkey,
    normalizeHotkeyAccelerator,
    setHotkeyReregisterCallback,
  });
}
