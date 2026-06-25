import type { ChildProcess } from "node:child_process";
import path from "node:path";
import { dialog } from "electron";
import type { ArkClient } from "@kosmos/ark";
import type { Instance } from "./instance";
import { keplerDataDir } from "./data-dir";
import { keplerLog } from "./logging";
import { createMainArkClientController } from "./main-ark-client-controller";
import { killBackendTree, spawnBackendProcess } from "./main-backend-process";

export { ARK_READY_REQUEST_TIMEOUT_MS } from "./main-ark-client-controller";

interface MainBackendSupervisorOptions {
  env: NodeJS.ProcessEnv;
  instance: Instance;
  resolveBackendExe(): string;
  getIsQuiting(): boolean;
  isUsageTrackerEnabled(): boolean;
  setupPomodoroNotifier(options: { arkClient: ArkClient }): void;
  teardownPomodoroNotifier(): void;
  setupFocusWidgetBackendSync(options: { arkClient: ArkClient }): void;
  teardownFocusWidgetBackendSync(): void;
  setupFocusSessionBackendSync(options: { arkClient: ArkClient }): void;
  teardownFocusSessionBackendSync(): void;
  setupDictationHotkey(): Promise<void>;
  setExtensionArkBridge(bridge: {
    request: ((req: Record<string, unknown>) => Promise<unknown>) | null;
    subscribe: ((event: string, handler: (event: unknown) => void) => () => void) | null;
  }): void;
  broadcastCommandsUpdated(): void;
  broadcastSettingsSyncUpdated(): void;
}

const BACKEND_RESPAWN_DELAYS_MS = [1000, 5000, 30_000, 60_000, 120_000];
const BACKEND_SUCCESSFUL_RUN_MS = 5 * 60 * 1000;
const BACKEND_MAX_CRASH_STREAK = BACKEND_RESPAWN_DELAYS_MS.length;

export interface MainBackendSupervisor {
  getArkClient(): ArkClient | null;
  getBackendLockPath(): string;
  isBackendRunning(): boolean;
  markBootInitStarted(): void;
  spawnBackend(): void;
  initArkClient(): Promise<void>;
  awaitArkReady(timeoutMs?: number): Promise<ArkClient>;
  recoverBackendIfDead(reason: string): Promise<void>;
  restartBackend(): Promise<void>;
  shutdown(): void;
}

export function createMainBackendSupervisor(
  options: MainBackendSupervisorOptions,
): MainBackendSupervisor {
  let backendProc: ChildProcess | null = null;
  let backendLockPath = "";
  let backendCrashStreak = 0;
  let backendStartedAt = 0;
  let backendRespawnTimer: NodeJS.Timeout | null = null;
  let backendCrashDialogShown = false;
  let bootInitStarted = false;
  let recoveringBackend = false;

  const arkController = createMainArkClientController({
    instance: options.instance,
    isBackendRunning,
    setupPomodoroNotifier: options.setupPomodoroNotifier,
    teardownPomodoroNotifier: options.teardownPomodoroNotifier,
    setupFocusWidgetBackendSync: options.setupFocusWidgetBackendSync,
    teardownFocusWidgetBackendSync: options.teardownFocusWidgetBackendSync,
    setupFocusSessionBackendSync: options.setupFocusSessionBackendSync,
    teardownFocusSessionBackendSync: options.teardownFocusSessionBackendSync,
    setupDictationHotkey: options.setupDictationHotkey,
    setExtensionArkBridge: options.setExtensionArkBridge,
    broadcastCommandsUpdated: options.broadcastCommandsUpdated,
    broadcastSettingsSyncUpdated: options.broadcastSettingsSyncUpdated,
  });

  function isBackendRunning(): boolean {
    return !!backendProc && !backendProc.killed;
  }

  function spawnBackend(): void {
    const spawned = spawnBackendProcess({
      env: options.env,
      instanceSlot: options.instance.slot,
      isUsageTrackerEnabled: options.isUsageTrackerEnabled,
      log: keplerLog,
      resolveBackendExe: options.resolveBackendExe,
    });
    if (!spawned.proc) return;

    backendProc = spawned.proc;
    backendLockPath = spawned.lockPath;
    backendStartedAt = Date.now();
    backendProc.on("exit", (code) => {
      const ranForMs = Date.now() - backendStartedAt;
      console.error(
        `[kepler-shell] backend exited code=${code} after ${(ranForMs / 1000).toFixed(1)}s`,
      );
      backendProc = null;
      void arkController.resetArkClient(`backend exited code=${code}`);
      if (options.getIsQuiting()) return;
      if (code === 0) return;
      scheduleBackendRespawn(ranForMs);
    });
  }

  function scheduleBackendRespawn(lastRanForMs: number): void {
    if (backendRespawnTimer) {
      clearTimeout(backendRespawnTimer);
      backendRespawnTimer = null;
    }
    if (lastRanForMs > BACKEND_SUCCESSFUL_RUN_MS) {
      backendCrashStreak = 0;
      backendCrashDialogShown = false;
    }
    if (backendCrashStreak >= BACKEND_MAX_CRASH_STREAK) {
      showBackendCrashDialog();
      return;
    }
    const delay = BACKEND_RESPAWN_DELAYS_MS[backendCrashStreak] ?? 120_000;
    backendCrashStreak += 1;
    console.error(
      `[kepler-shell] supervisor: respawn attempt ${backendCrashStreak}/${BACKEND_MAX_CRASH_STREAK} in ${delay / 1000}s`,
    );
    backendRespawnTimer = setTimeout(() => {
      backendRespawnTimer = null;
      if (options.getIsQuiting()) return;
      keplerLog.warn("supervisor", "respawning backend", { streak: backendCrashStreak });
      spawnBackend();
      void arkController.initArkClient();
    }, delay);
  }

  function showBackendCrashDialog(): void {
    if (backendCrashDialogShown) return;
    backendCrashDialogShown = true;
    keplerLog.error("supervisor", "backend crashed too many times in a row, giving up", {
      streak: backendCrashStreak,
    });
    const crashesDir = path.join(keplerDataDir(), "crashes");
    void dialog
      .showMessageBox({
        type: "error",
        title: "Kosmos — runtime не запускается",
        message: "Kosmos Runtime упал 5 раз подряд. Автоматический перезапуск приостановлен.",
        detail:
          "Откройте Настройки → Диагностика и посмотрите последние crash-логи.\n\n" +
          `Папка с отчётами: ${crashesDir}\n\n` +
          "После проверки попробуйте: Настройки → Перезапустить backend.",
        buttons: ["OK"],
        defaultId: 0,
      })
      .catch((e) => {
        keplerLog.error("supervisor", "dialog failed", { err: String(e) });
      });
  }

  async function recoverBackendIfDead(reason: string): Promise<void> {
    if (!bootInitStarted || recoveringBackend || options.getIsQuiting()) return;
    if (
      arkController.isInitInFlight() ||
      arkController.hasPendingInitRetry() ||
      backendRespawnTimer
    ) {
      return;
    }
    recoveringBackend = true;
    try {
      if (await arkController.backendHealthy()) return;
      keplerLog.warn("supervisor", "backend unhealthy — recovering", { reason });
      backendCrashStreak = 0;
      backendCrashDialogShown = false;
      await arkController.resetArkClient(`recover: ${reason}`);
      if (backendProc && !backendProc.killed) {
        try {
          backendProc.kill();
        } catch {
          // уже мёртв — ок
        }
      }
      spawnBackend();
      await arkController.initArkClient();
    } finally {
      recoveringBackend = false;
    }
  }

  async function restartBackend(): Promise<void> {
    backendCrashStreak = 0;
    backendCrashDialogShown = false;
    if (backendRespawnTimer) {
      clearTimeout(backendRespawnTimer);
      backendRespawnTimer = null;
    }
    await arkController.resetArkClient("backend restart");
    if (backendProc && !backendProc.killed) {
      backendProc.kill();
    }
    spawnBackend();
    void arkController.initArkClient();
  }

  function shutdown(): void {
    arkController.shutdown();
    if (backendProc && !backendProc.killed) {
      killBackendTree(backendProc);
    }
  }

  return {
    getArkClient: () => arkController.getArkClient(),
    getBackendLockPath: () => backendLockPath,
    isBackendRunning,
    markBootInitStarted: () => {
      bootInitStarted = true;
    },
    spawnBackend,
    initArkClient: () => arkController.initArkClient(),
    awaitArkReady: (timeoutMs) => arkController.awaitArkReady(timeoutMs),
    recoverBackendIfDead,
    restartBackend,
    shutdown,
  };
}
