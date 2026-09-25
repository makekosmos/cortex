import { BrowserWindow, globalShortcut, nativeTheme, powerMonitor } from "electron";
import type { ArkClient } from "@kosmos/ark";
import type { JsonRecord } from "./extension-permissions";
import { openHostedApp } from "./host-app";
import { openManager } from "./manager-navigation";
import { openTestHarnessWindow } from "./main-test-window";
import { registerMainProtocols } from "./main-protocols";
import { setupAutoUpdater } from "./autoupdater-host";
import {
  getStoredHotkey,
  normalizeHotkeyAccelerator,
  setHotkeyReregisterCallback,
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

interface RunAppReadyOptions {
  awaitArkReady: Parameters<typeof registerMainProtocols>[0]["awaitArkReady"];
  backendSupervisor: AppReadyBackendSupervisor;
  instance: AppReadyInstance;
  log: AppReadyLogger;
  runBootSelfCheck(): void;
  recoverLegacyMigration?(): Promise<void>;
  runLegacyMigration?(): Promise<void>;
}

interface RegisterGlobalHotkeyOptions {
  slot: string;
  slotHotkey: string | null;
  getStoredHotkey: () => string;
  normalizeHotkeyAccelerator: (accelerator: string) => string | null;
  setHotkeyReregisterCallback: (callback: (accelerator: string) => boolean) => void;
}

function registerGlobalHotkey({
  getStoredHotkey: readStoredHotkey,
  normalizeHotkeyAccelerator: normalizeAccelerator,
  setHotkeyReregisterCallback: setReregisterCallback,
  slot,
  slotHotkey,
}: RegisterGlobalHotkeyOptions): void {
  // Anti-repeat по delta-времени между fire'ами. Windows key-repeat шлёт
  // WM_HOTKEY каждые ~33 мс пока сочетание зажато; тап-тап (с реальным
  // отпусканием пробела) даёт паузу >>=100 мс. Threshold 80 мс отрезает
  // auto-repeat но пропускает быстрые тапы (>12 Hz всё равно бывает редко).
  // Глобальный accelerator при этом всегда зарегистрирован - Windows не
  // выдаёт Alt+Space в системные меню окна.
  let lastFireAt = 0;
  const openShell = () => {
    const now = Date.now();
    const gap = now - lastFireAt;
    lastFireAt = now;
    if (gap < 80) return;
    if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") return;
    void openHostedApp("com.kosmos.shell");
  };

  // Slot'ы без hotkey (dev-<x>, test-<x>) - пропускаем регистрацию вовсе.
  // Пользователь открывает shell через tray click. Это критично для
  // multi-dev: два инстанса не могут поделить один accelerator, второй
  // молча проиграл бы Windows OS race.
  if (slotHotkey !== null) {
    let currentAccelerator = readStoredHotkey();
    function tryRegister(accelerator: string): boolean {
      const normalized = normalizeAccelerator(accelerator);
      if (!normalized) return false;
      try {
        if (globalShortcut.isRegistered(currentAccelerator)) {
          globalShortcut.unregister(currentAccelerator);
        }
        const registered = globalShortcut.register(normalized, openShell);
        if (registered) {
          currentAccelerator = normalized;
          console.log(`[kepler-shell] globalShortcut ${normalized} registered`);
          return true;
        }
        // Откатываемся на предыдущий, если новая регистрация не удалась.
        globalShortcut.register(currentAccelerator, openShell);
        return false;
      } catch (e) {
        console.error("[kepler-shell] globalShortcut register error:", e);
        return false;
      }
    }
    const ok = tryRegister(currentAccelerator);
    setReregisterCallback(tryRegister);
    if (!ok) {
      console.error(`[kepler-shell] globalShortcut ${currentAccelerator} register failed`);
    }
  } else {
    console.log(`[kepler-shell] hotkey disabled for slot ${slot} - use tray click`);
  }

  // F12 toggle DevTools (dev mode только) - глобальный hotkey удобнее чем
  // accelerator menu. Windows route'ит global accelerator к фокусному окну;
  // в multi-dev только активное окно получит toggle.
  if (process.env.VITE_DEV_SERVER_URL) {
    globalShortcut.register("F12", () => {
      BrowserWindow.getFocusedWindow()?.webContents.toggleDevTools();
    });
  }
}

export async function runAppReady({
  awaitArkReady,
  backendSupervisor,
  instance,
  log,
  runBootSelfCheck,
  recoverLegacyMigration,
  runLegacyMigration,
}: RunAppReadyOptions): Promise<void> {
  runBootSelfCheck();
  nativeTheme.themeSource = "dark";

  if (process.argv.includes("--autostart")) {
    console.log(
      "[kepler-shell] launched from Windows autorun (--autostart marker present); shell remains silent, tray icon only",
    );
  }

  backendSupervisor.spawnBackend();
  registerMainProtocols({ awaitArkReady });
  backendSupervisor.markBootInitStarted();
  const boot = backendSupervisor.initArkClient();
  if (!process.argv.includes("--autostart")) {
    // В test mode Playwright нужен любой renderer с `window.kepler` bridge —
    // открываем скрытое harness-окно вместо Manager.
    const openSurface = process.env.KOSMOS_TEST_MODE === "1" ? openTestHarnessWindow : openManager;
    openSurface();
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

  registerGlobalHotkey({
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
