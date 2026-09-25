import { beforeEach, expect, mock, test } from "../test-support/node-test.mjs";
import type { JsonRecord } from "./extension-permissions";

const engineLock = {
  format_version: 1,
  api_version: { major: 1, minor: 0, patch: 0 },
  pid: 42,
  http_port: 4317,
  ws_port: 4318,
  auth_token: "a".repeat(64),
  started_at: "2026-07-29T00:00:00Z",
  correlation_id: "12345678-1234-4234-8234-123456789abc",
};

type TestState =
  | { kind: "connected"; lock: typeof engineLock }
  | { kind: "launch-failed"; error: { kind: "launch-failed"; message: string } };
let nextState: TestState = { kind: "connected", lock: engineLock };
let capturedOptions: JsonRecord | null = null;
const invokedOperations: JsonRecord[] = [];
let invokeError: Error | null = null;

class FakeArkClient {
  commands = { list: async () => [] };
  constructor(options: JsonRecord) {
    capturedOptions = options;
  }
  async start(): Promise<void> {}
  async stop(): Promise<void> {}
  async invokeOperation(request: JsonRecord): Promise<JsonRecord> {
    invokedOperations.push(request);
    if (invokeError) throw invokeError;
    return { ok: true };
  }
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
  ensureEngineRunning: async () => nextState,
}));
mock.module("./data-dir", () => ({ keplerDataDir: () => "C:\\Kosmos-test" }));
mock.module("./logging", () => ({
  keplerLog: { error: mock.fn(), info: mock.fn(), warn: mock.fn(), setCorrelationId: mock.fn() },
}));
mock.module("./main-protocols", () => ({ clearMainProtocolCaches: mock.fn() }));
mock.module("./extension-ark-ipc", () => ({ setExtensionArkBridge: mock.fn() }));

const { createMainArkClientController } = await import("./main-ark-client-controller");

function createController() {
  return createMainArkClientController({
    desktopAuthorityCredential: "private-credential",
    // SAFETY: The test supplies the minimal instance shape consumed by the controller.
    instance: { slot: "test" } as never,
    isBackendRunning: () => true,
    setupDictationHotkey: async () => {},
    broadcastCommandsUpdated: () => {},
  });
}

beforeEach(() => {
  nextState = { kind: "connected", lock: engineLock };
  capturedOptions = null;
  invokedOperations.length = 0;
  invokeError = null;
});

test("Desktop creates ArkClient with the Engine lock", { concurrency: false }, async () => {
  const controller = createController();
  await controller.initArkClient();

  expect(capturedOptions?.engineLock).toEqual(engineLock);
  expect(capturedOptions?.keplerLock).toBeUndefined();
  expect(capturedOptions?.engineClientClass).toBe("desktop");
  expect(capturedOptions?.engineClientVersion).toBe("9.8.7");
  expect(invokedOperations).toEqual([
    { operation: "desktop.authority.bind", params: { credential: "private-credential" } },
  ]);
});

test("Desktop fails closed when Engine cannot launch", { concurrency: false }, async () => {
  nextState = {
    kind: "launch-failed",
    error: { kind: "launch-failed", message: "Engine did not start" },
  };
  const controller = createController();
  await controller.initArkClient();

  expect(controller.getArkClient()).toBeNull();
  expect(capturedOptions).toBeNull();
});

test(
  "Desktop is not ready when the private authority lease is rejected",
  { concurrency: false },
  async () => {
    invokeError = new Error("desktop authority denied");
    const controller = createController();
    const ready = controller.awaitArkReady().catch((error: Error) => error);
    await controller.initArkClient();

    expect(controller.getArkClient()).toBeNull();
    expect((await ready).message).toContain("desktop authority denied");
  },
);
