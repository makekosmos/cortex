import { app, BrowserWindow } from "electron";
import { ArkClient, ensureEngineRunning } from "@kosmos/ark";
import type { EngineLockInfo, EngineState } from "@kosmos/ark";
import type { Instance } from "./instance";
import { keplerDataDir } from "./data-dir";
import { keplerLog } from "./logging";
import { clearMainProtocolCaches } from "./main-protocols";
type ArkRendererEvent = Parameters<Parameters<ArkClient["onArkEvent"]>[0]>[0];

interface MainArkClientControllerOptions {
  instance: Instance;
  isBackendRunning(): boolean;
  setupPomodoroNotifier(options: { arkClient: ArkClient }): void;
  teardownPomodoroNotifier(): void;
  setupFocusWidgetBackendSync(options: { arkClient: ArkClient }): void;
  teardownFocusWidgetBackendSync(): void;
  setupFocusSessionBackendSync(options: { arkClient: ArkClient }): void;
  teardownFocusSessionBackendSync(): void | Promise<void>;
  setupDictationHotkey(): Promise<void>;
  broadcastCommandsUpdated(): void;
}

const ARK_CLIENT_LOCK_WAIT_MS = 30_000;
const ARK_INIT_RETRY_DELAYS_MS = [1000, 2000, 5000, 10_000];
export const ARK_READY_REQUEST_TIMEOUT_MS =
  ARK_CLIENT_LOCK_WAIT_MS + (ARK_INIT_RETRY_DELAYS_MS[0] ?? 0) + ARK_CLIENT_LOCK_WAIT_MS + 5_000;
const ARK_REQUEST_TIMEOUT_MS = 60 * 1000;
const KEPLER_SPACE_ID = "kepler-default";

export interface MainArkClientController {
  awaitArkReady(timeoutMs?: number): Promise<ArkClient>;
  backendHealthy(timeoutMs?: number): Promise<boolean>;
  getArkClient(): ArkClient | null;
  hasPendingInitRetry(): boolean;
  initArkClient(): Promise<void>;
  isInitInFlight(): boolean;
  resetArkClient(reason: string): Promise<void>;
  shutdown(): Promise<void>;
}

export function createMainArkClientController(
  options: MainArkClientControllerOptions,
): MainArkClientController {
  let arkClient: ArkClient | null = null;
  let arkRendererEventsUnsubscribe: (() => void) | null = null;
  let arkInitRetryTimer: NodeJS.Timeout | null = null;
  let arkInitRetryAttempt = 0;
  let arkInitInFlight = false;
  let arkClientReady: Promise<ArkClient> | null = null;
  let arkClientReadyResolve: ((c: ArkClient) => void) | null = null;
  let arkClientReadyReject: ((e: Error) => void) | null = null;
  let lastEngineFailure: EngineState["kind"] | null = null;

  function broadcastBackendEvent(
    event: "kepler:backend:ready" | "kepler:backend:disconnected",
  ): void {
    for (const w of BrowserWindow.getAllWindows()) {
      if (w.isDestroyed()) continue;
      try {
        w.webContents.send(event);
      } catch {
        // окно может быть в процессе destroy; игнор.
      }
    }
  }

  function broadcastArkRendererEvent(event: ArkRendererEvent): void {
    for (const win of BrowserWindow.getAllWindows()) {
      if (win.isDestroyed()) continue;
      try {
        win.webContents.send("kepler:ark:event", event);
      } catch {
        /* ignore */
      }
    }
  }

  function ensureArkReadyPromise(): Promise<ArkClient> {
    if (arkClient) return Promise.resolve(arkClient);
    if (arkClientReady) return arkClientReady;
    arkClientReady = new Promise<ArkClient>((resolve, reject) => {
      arkClientReadyResolve = resolve;
      arkClientReadyReject = reject;
    });
    return arkClientReady;
  }

  async function resetArkClient(reason: string): Promise<void> {
    const wasConnected = arkClient !== null;
    const err = new Error(`ArkClient reset: ${reason}`);
    if (arkInitRetryTimer) {
      clearTimeout(arkInitRetryTimer);
      arkInitRetryTimer = null;
    }
    arkInitRetryAttempt = 0;
    arkClientReadyReject?.(err);
    arkClientReadyResolve = null;
    arkClientReadyReject = null;
    arkClientReady = null;
    const prev = arkClient;
    arkClient = null;
    clearMainProtocolCaches();
    arkRendererEventsUnsubscribe?.();
    arkRendererEventsUnsubscribe = null;
    if (wasConnected) broadcastBackendEvent("kepler:backend:disconnected");
    options.teardownPomodoroNotifier();
    options.teardownFocusWidgetBackendSync();
    await options.teardownFocusSessionBackendSync();
    if (prev) {
      try {
        await prev.stop();
      } catch (e) {
        keplerLog.warn("ark", "ArkClient stop failed", { err: String(e) });
      }
    }
  }

  function scheduleArkClientInitRetry(reason: string): void {
    if (arkClient || arkInitRetryTimer) return;
    if (!options.isBackendRunning()) return;
    const delay = ARK_INIT_RETRY_DELAYS_MS[arkInitRetryAttempt];
    if (delay === undefined) {
      keplerLog.error("ark", "ArkClient init retry exhausted", { reason });
      return;
    }
    arkInitRetryAttempt += 1;
    arkInitRetryTimer = setTimeout(() => {
      arkInitRetryTimer = null;
      void initArkClient();
    }, delay);
    keplerLog.warn("ark", "scheduled ArkClient init retry", { reason, delayMs: delay });
  }

  async function awaitArkReady(timeoutMs = ARK_READY_REQUEST_TIMEOUT_MS): Promise<ArkClient> {
    if (arkClient) return arkClient;
    const p = ensureArkReadyPromise();
    let timer: NodeJS.Timeout | null = null;
    try {
      return await Promise.race([
        p,
        new Promise<ArkClient>((_, rej) => {
          timer = setTimeout(() => rej(new Error("ArkClient not ready (timeout)")), timeoutMs);
        }),
      ]);
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  async function initArkClient(): Promise<void> {
    arkInitInFlight = true;
    try {
      ensureArkReadyPromise();
      const state = await ensureEngineRunning({
        appDataPath: app.getPath("appData"),
        dataDir: keplerDataDir(),
        waitMs: ARK_CLIENT_LOCK_WAIT_MS,
        autoLaunch: false,
      });
      if (state.kind !== "connected") {
        if (lastEngineFailure !== state.kind) {
          keplerLog.error("ark", `Kepler: ${state.kind}`, {
            classification: state.kind,
          });
          lastEngineFailure = state.kind;
        }
        scheduleArkClientInitRetry(state.kind);
        return;
      }
      const engineLock: EngineLockInfo = state.lock;
      lastEngineFailure = null;
      const client = new ArkClient({
        spaceId: KEPLER_SPACE_ID,
        deviceId: `kepler-shell-${options.instance.slot}`,
        deviceName: "Kosmos Desktop",
        engineLock,
        engineClientClass: "desktop",
        engineClientVersion: app.getVersion(),
        requestTimeoutMs: ARK_REQUEST_TIMEOUT_MS,
      });
      await client.start();
      arkClient = client;
      arkInitRetryAttempt = 0;
      arkClientReadyResolve?.(client);
      broadcastBackendEvent("kepler:backend:ready");
      keplerLog.info("ark", "ArkClient connected to Engine", {
        enginePid: engineLock.pid,
        wsPort: engineLock.ws_port,
      });
      setupConnectedClient(client);
    } catch (e) {
      keplerLog.error("ark", "ArkClient init failed", { err: String(e) });
      arkClientReadyReject?.(e instanceof Error ? e : new Error(String(e)));
    } finally {
      arkInitInFlight = false;
    }
  }

  function setupConnectedClient(client: ArkClient): void {
    try {
      options.setupPomodoroNotifier({ arkClient: client });
    } catch (e) {
      keplerLog.error("pomodoro-notifier", "setup failed", { err: String(e) });
    }
    try {
      options.setupFocusWidgetBackendSync({ arkClient: client });
    } catch (e) {
      keplerLog.error("focus-widget", "backend sync setup failed", { err: String(e) });
    }
    try {
      options.setupFocusSessionBackendSync({ arkClient: client });
    } catch (e) {
      keplerLog.error("focus-session", "backend sync setup failed", { err: String(e) });
    }
    void options
      .setupDictationHotkey()
      .catch((e) => keplerLog.error("dictation", "hotkey setup failed", { err: String(e) }));
    arkRendererEventsUnsubscribe?.();
    arkRendererEventsUnsubscribe = client.onArkEvent(broadcastArkRendererEvent);
    client.commands.onChanged(() => {
      options.broadcastCommandsUpdated();
    });
  }

  async function backendHealthy(timeoutMs = 1500): Promise<boolean> {
    const client = arkClient;
    if (!client) return false;
    let timer: NodeJS.Timeout | null = null;
    try {
      await Promise.race([
        client.commands.list(),
        new Promise<never>((_, rej) => {
          timer = setTimeout(() => rej(new Error("health probe timeout")), timeoutMs);
        }),
      ]);
      return true;
    } catch {
      return false;
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  async function shutdown(): Promise<void> {
    await options.teardownFocusSessionBackendSync();
    options.teardownPomodoroNotifier();
    if (arkClient) {
      try {
        await arkClient.stop();
      } catch (e) {
        console.error("[kepler-shell] arkClient stop error:", e);
      }
      arkClient = null;
    }
  }

  return {
    awaitArkReady,
    backendHealthy,
    getArkClient: () => arkClient,
    hasPendingInitRetry: () => arkInitRetryTimer !== null,
    initArkClient,
    isInitInFlight: () => arkInitInFlight,
    resetArkClient,
    shutdown,
  };
}
