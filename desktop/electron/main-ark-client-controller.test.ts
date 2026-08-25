import { beforeEach, expect, mock, test } from "bun:test";
import type { JsonRecord } from "./extension-permissions";

const engineLock = {
  format_version: 1,
  protocol_version: { major: 1, minor: 0, patch: 0 },
  pid: 42,
  ws_port: 4318,
  auth_token: "a".repeat(64),
  started_at: "2026-07-29T00:00:00Z",
  db_path: "C:\\Kosmos-test\\data.db",
};

type TestState =
  | { kind: "connected"; lock: typeof engineLock }
  | { kind: "launch-failed"; reason: string };
let nextState: TestState = { kind: "connected", lock: engineLock };
let capturedOptions: JsonRecord | null = null;

class FakeArkClient {
  commands = { list: async () => [] };
  constructor(options: JsonRecord) {
    capturedOptions = options;
  }
  async start(): Promise<void> {}
  async stop(): Promise<void> {}
  onArkEvent(): () => void {
    return () => {};
  }
}

mock.module("electron", () => ({
  app: { getPath: () => "C:\\Kosmos-test", getVersion: () => "9.8.7" },
  BrowserWindow: { getAllWindows: () => [] },
  ipcRenderer: {
    invoke: async () => undefined,
    on: () => {},
    removeListener: () => {},
    send: () => {},
  },
}));
mock.module("@kosmos/ark", () => ({
  ArkClient: FakeArkClient,
  ensureKeplerRunning: async () => nextState,
}));
mock.module("./data-dir", () => ({ keplerDataDir: () => "C:\\Kosmos-test" }));
mock.module("./logging", () => ({
  keplerLog: { error: mock(), info: mock(), warn: mock(), setCorrelationId: mock() },
}));
mock.module("./main-protocols", () => ({ clearMainProtocolCaches: mock() }));

const { createMainArkClientController } = await import("./main-ark-client-controller");

function createController() {
  return createMainArkClientController({
    // SAFETY: The test supplies the minimal instance shape consumed by the controller.
    instance: { slot: "test" } as never,
    isBackendRunning: () => true,
    setupPomodoroNotifier: () => {},
    teardownPomodoroNotifier: () => {},
    setupFocusWidgetBackendSync: () => {},
    teardownFocusWidgetBackendSync: () => {},
    setupFocusSessionBackendSync: () => {},
    teardownFocusSessionBackendSync: () => {},
    setupDictationHotkey: async () => {},
    broadcastCommandsUpdated: () => {},
  });
}

beforeEach(() => {
  nextState = { kind: "connected", lock: engineLock };
  capturedOptions = null;
});

test("Desktop creates ArkClient with the Kepler lock", async () => {
  const controller = createController();
  await controller.initArkClient();

  expect(capturedOptions?.keplerLock).toEqual(engineLock);
  expect(capturedOptions?.engineLock).toBeUndefined();
  expect(capturedOptions?.engineClientClass).toBeUndefined();
  expect(capturedOptions?.engineClientVersion).toBeUndefined();
});

test("Desktop fails closed when Kepler cannot launch", async () => {
  nextState = {
    kind: "launch-failed",
    reason: "Kepler did not start",
  };
  const controller = createController();
  await controller.initArkClient();

  expect(controller.getArkClient()).toBeNull();
  expect(capturedOptions).toBeNull();
});
