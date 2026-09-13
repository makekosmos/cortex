import { EventEmitter } from "node:events";
import { beforeEach, expect, mock, test } from "../test-support/node-test.mjs";

class FakeProcess extends EventEmitter {
  killed = false;

  kill(): boolean {
    this.killed = true;
    return true;
  }
}

const spawnedProcesses: FakeProcess[] = [];
let resetCalls = 0;
let restartCalls = 0;
let lockProcessAlive = false;
let controllerCredential = "";
let spawnedCredential = "";
let backendExitCalls: number[] = [];
const controller = {
  awaitArkReady: async () => ({}),
  backendHealthy: async () => false,
  getArkClient: () => null,
  hasPendingInitRetry: () => false,
  initArkClient: async () => {},
  isInitInFlight: () => false,
  resetArkClient: async () => {
    resetCalls += 1;
  },
  shutdown: () => {},
};

mock.module("electron", () => ({
  dialog: { showMessageBox: async () => ({ response: 0 }) },
}));
mock.module("./data-dir", () => ({ keplerDataDir: () => "C:\\Kosmos-test" }));
mock.module("./logging", () => ({
  keplerLog: { error: () => {}, info: () => {}, warn: () => {} },
}));
mock.module("./main-ark-client-controller", () => ({
  ARK_READY_REQUEST_TIMEOUT_MS: 1,
  createMainArkClientController: (options: { desktopAuthorityCredential: string }) => {
    controllerCredential = options.desktopAuthorityCredential;
    return controller;
  },
}));
mock.module("./main-backend-process", () => ({
  isBackendLockProcessAlive: () => lockProcessAlive,
  restartBackendProcess: async () => {
    restartCalls += 1;
  },
  spawnBackendProcess: (options: { desktopAuthorityCredential: string }) => {
    spawnedCredential = options.desktopAuthorityCredential;
    const proc = new FakeProcess();
    spawnedProcesses.push(proc);
    return { proc, lockPath: "C:\\Kosmos-test\\kepler.lock.json" };
  },
}));

const { BACKEND_TRAY_EXIT_CODE, createMainBackendSupervisor } =
  await import("./main-backend-supervisor");

beforeEach(() => {
  spawnedProcesses.length = 0;
  resetCalls = 0;
  restartCalls = 0;
  lockProcessAlive = false;
  controllerCredential = "";
  spawnedCredential = "";
  backendExitCalls = [];
});

function createTestSupervisor(
  getIsQuiting: () => boolean,
  onBackendExit: (code: number | null) => void = () => {},
) {
  return createMainBackendSupervisor({
    env: {},
    // SAFETY: The surrounding boundary establishes this documented contract.
    instance: { slot: "test" } as never,
    resolveBackendExe: () => "kepler-backend.exe",
    getIsQuiting,
    setupPomodoroNotifier: () => {},
    teardownPomodoroNotifier: () => {},
    setupFocusWidgetBackendSync: () => {},
    teardownFocusWidgetBackendSync: () => {},
    setupFocusSessionBackendSync: () => {},
    teardownFocusSessionBackendSync: () => {},
    setupDictationHotkey: async () => {},
    broadcastCommandsUpdated: () => {},
    onBackendExit,
  });
}

test("recovery restarts the native core and ensures its supervisor", async () => {
  const supervisor = createTestSupervisor(() => false);
  supervisor.markBootInitStarted();
  supervisor.spawnBackend();
  await supervisor.recoverBackendIfDead("regression-test");

  expect(spawnedProcesses).toHaveLength(2);
  expect(restartCalls).toBe(1);
  expect(supervisor.isBackendRunning()).toBe(true);
  expect(resetCalls).toBe(1);
  expect(spawnedCredential).toBe(controllerCredential);
  expect(spawnedCredential).toMatch(/^[0-9a-f]{64}$/);
});

test("Electron shutdown does not kill the independent engine", () => {
  const supervisor = createTestSupervisor(() => true);
  supervisor.spawnBackend();
  const engine = spawnedProcesses[0]!;

  supervisor.shutdown();

  expect(engine.killed).toBe(false);
});

test("idempotent ensure may exit while the lock-owned core stays alive", async () => {
  const supervisor = createTestSupervisor(
    () => true,
    (code) => {
      if (code === BACKEND_TRAY_EXIT_CODE) backendExitCalls.push(code);
    },
  );
  supervisor.spawnBackend();
  lockProcessAlive = true;

  spawnedProcesses[0]!.emit("exit", 0);
  await Promise.resolve();

  expect(supervisor.isBackendRunning()).toBe(true);
  expect(resetCalls).toBe(0);
  expect(backendExitCalls).toEqual([]);
});

test("tray backend exit notifies Electron so the UI can quit with it", () => {
  expect(BACKEND_TRAY_EXIT_CODE).toBe(42);
  const supervisor = createTestSupervisor(
    () => false,
    (code) => {
      if (code === BACKEND_TRAY_EXIT_CODE) backendExitCalls.push(code);
    },
  );
  supervisor.spawnBackend();

  spawnedProcesses[0]!.emit("exit", BACKEND_TRAY_EXIT_CODE);

  expect(backendExitCalls).toEqual([BACKEND_TRAY_EXIT_CODE]);
});

test("external shutdown exit does not ask Electron to quit", () => {
  const supervisor = createTestSupervisor(
    () => false,
    (code) => {
      if (code === BACKEND_TRAY_EXIT_CODE) backendExitCalls.push(code);
    },
  );
  supervisor.spawnBackend();

  spawnedProcesses[0]!.emit("exit", 0);

  expect(backendExitCalls).toEqual([]);
});
