import { EventEmitter } from "node:events";
import { beforeEach, expect, mock, test } from "bun:test";

class FakeProcess extends EventEmitter {
  killed = false;

  kill(): boolean {
    this.killed = true;
    return true;
  }
}

const spawnedProcesses: FakeProcess[] = [];
let resetCalls = 0;
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
  createMainArkClientController: () => controller,
}));
mock.module("./main-backend-process", () => ({
  killBackendTree: () => {},
  spawnBackendProcess: () => {
    const proc = new FakeProcess();
    spawnedProcesses.push(proc);
    return { proc, lockPath: "C:\\Kosmos-test\\kepler.lock.json" };
  },
}));

const { createMainBackendSupervisor } = await import("./main-backend-supervisor");

beforeEach(() => {
  spawnedProcesses.length = 0;
  resetCalls = 0;
});

function createTestSupervisor(getIsQuiting: () => boolean) {
  return createMainBackendSupervisor({
    env: {},
    instance: { slot: "test" } as never,
    resolveBackendExe: () => "kepler-backend.exe",
    getIsQuiting,
    isUsageTrackerEnabled: () => false,
    setupPomodoroNotifier: () => {},
    teardownPomodoroNotifier: () => {},
    setupFocusWidgetBackendSync: () => {},
    teardownFocusWidgetBackendSync: () => {},
    setupFocusSessionBackendSync: () => {},
    teardownFocusSessionBackendSync: () => {},
    setupDictationHotkey: async () => {},
    setExtensionArkBridge: () => {},
    broadcastCommandsUpdated: () => {},
    broadcastSettingsSyncUpdated: () => {},
  });
}

test("late exit from replaced backend keeps the active replacement", async () => {
  let quitting = false;
  const supervisor = createTestSupervisor(() => quitting);

  supervisor.markBootInitStarted();
  supervisor.spawnBackend();
  const replaced = spawnedProcesses[0]!;
  await supervisor.recoverBackendIfDead("regression-test");
  expect(spawnedProcesses).toHaveLength(2);
  expect(supervisor.isBackendRunning()).toBe(true);
  expect(resetCalls).toBe(1);

  // Regression: 2026-07-15. The old process can emit exit after its replacement is active.
  quitting = true;
  replaced.emit("exit", null);
  await Promise.resolve();

  expect(supervisor.isBackendRunning()).toBe(true);
  expect(resetCalls).toBe(1);
});

test("exit from the active backend still clears its process slot", async () => {
  const supervisor = createTestSupervisor(() => true);
  supervisor.spawnBackend();

  spawnedProcesses[0]!.emit("exit", 1);
  await Promise.resolve();

  expect(supervisor.isBackendRunning()).toBe(false);
  expect(resetCalls).toBe(1);
});
