import { beforeEach, expect, mock, test } from "bun:test";

const engineLock = {
  format_version: 1,
  protocol_version: { major: 1, minor: 0, patch: 0 },
  pid: 42,
  ws_port: 4318,
  auth_token: "a".repeat(64),
  started_at: "2026-07-29T00:00:00Z",
  db_path: "C:\\Kosmos-test\\data.db",
};

let nextState: unknown = { kind: "connected", lock: engineLock };
let capturedOptions: Record<string, unknown> | null = null;

class FakeArkClient {
  commands = { list: async () => [] };
  constructor(options: Record<string, unknown>) {
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
