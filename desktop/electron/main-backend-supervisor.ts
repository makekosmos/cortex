import type { ChildProcess } from "node:child_process";
import { randomBytes } from "node:crypto";
import type { ArkClient } from "@kosmos/ark";
import type { Instance } from "./instance";
import { keplerLog } from "./logging";
import { createMainArkClientController } from "./main-ark-client-controller";
import {
  isBackendLockProcessAlive,
  restartBackendProcess,
  spawnBackendProcess,
} from "./main-backend-process";

export { ARK_READY_REQUEST_TIMEOUT_MS } from "./main-ark-client-controller";

interface MainBackendSupervisorOptions {
  env: NodeJS.ProcessEnv;
  instance: Instance;
  resolveBackendExe(): string;
  getIsQuiting(): boolean;
  setupPomodoroNotifier(options: { arkClient: ArkClient }): void;
  teardownPomodoroNotifier(): void;
  setupFocusWidgetBackendSync(options: { arkClient: ArkClient }): void;
  teardownFocusWidgetBackendSync(): void;
  setupFocusSessionBackendSync(options: { arkClient: ArkClient }): void;
  teardownFocusSessionBackendSync(): void | Promise<void>;
  setupDictationHotkey(): Promise<void>;
  broadcastCommandsUpdated(): void;
}

export interface MainBackendSupervisor {
  getArkClient(): ArkClient | null;
  isBackendRunning(): boolean;
  markBootInitStarted(): void;
  spawnBackend(): void;
  initArkClient(): Promise<void>;
  awaitArkReady(timeoutMs?: number): Promise<ArkClient>;
  recoverBackendIfDead(reason: string): Promise<void>;
  restartBackend(): Promise<void>;
  shutdown(): Promise<void>;
}

export function createMainBackendSupervisor(
  options: MainBackendSupervisorOptions,
): MainBackendSupervisor {
  const desktopAuthorityCredential = randomBytes(32).toString("hex");
  let backendProc: ChildProcess | null = null;
  let backendLockPath = "";
  let bootInitStarted = false;
  let recoveringBackend = false;

  const arkController = createMainArkClientController({
    desktopAuthorityCredential,
    instance: options.instance,
    isBackendRunning,
    setupPomodoroNotifier: options.setupPomodoroNotifier,
    teardownPomodoroNotifier: options.teardownPomodoroNotifier,
    setupFocusWidgetBackendSync: options.setupFocusWidgetBackendSync,
    teardownFocusWidgetBackendSync: options.teardownFocusWidgetBackendSync,
    setupFocusSessionBackendSync: options.setupFocusSessionBackendSync,
    teardownFocusSessionBackendSync: options.teardownFocusSessionBackendSync,
    setupDictationHotkey: options.setupDictationHotkey,
    broadcastCommandsUpdated: options.broadcastCommandsUpdated,
  });

  function isBackendRunning(): boolean {
    return (!!backendProc && !backendProc.killed) || isBackendLockProcessAlive(backendLockPath);
  }

  function spawnBackend(): void {
    const spawned = spawnBackendProcess({
      desktopAuthorityCredential,
      env: options.env,
      instanceSlot: options.instance.slot,
      log: keplerLog,
      resolveBackendExe: options.resolveBackendExe,
    });
    if (!spawned.proc) return;

    const proc = spawned.proc;
    backendProc = proc;
    backendLockPath = spawned.lockPath;
    proc.on("exit", (code) => {
      if (backendProc !== proc) return;
      backendProc = null;
      if (code !== 0 && !options.getIsQuiting()) {
        void arkController.resetArkClient(`engine supervisor exited code=${code}`);
      }
    });
  }

  async function recoverBackendIfDead(reason: string): Promise<void> {
    if (!bootInitStarted || recoveringBackend || options.getIsQuiting()) return;
    if (arkController.isInitInFlight() || arkController.hasPendingInitRetry()) {
      return;
    }
    recoveringBackend = true;
    try {
      if (await arkController.backendHealthy()) return;
      keplerLog.warn("supervisor", "backend unhealthy — recovering", { reason });
      await arkController.resetArkClient(`recover: ${reason}`);
      await restartBackendProcess(options.resolveBackendExe);
      spawnBackend();
      await arkController.initArkClient();
    } finally {
      recoveringBackend = false;
    }
  }

  async function restartBackend(): Promise<void> {
    await arkController.resetArkClient("backend restart");
    await restartBackendProcess(options.resolveBackendExe);
    spawnBackend();
    void arkController.initArkClient();
  }

  function shutdown(): Promise<void> {
    return arkController.shutdown();
  }

  return {
    getArkClient: () => arkController.getArkClient(),
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
