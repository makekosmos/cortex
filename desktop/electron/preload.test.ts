import { expect, mock, test } from "../test-support/node-test.mjs";

const invocations: unknown[][] = [];
mock.module("electron", () => ({
  ipcRenderer: {
    invoke: async (...args: unknown[]) => {
      invocations.push(args);
      return { running: true, pid: 42, wsPort: 4318, lockFilePath: "" };
    },
    on: () => {},
    removeListener: () => {},
    send: () => {},
  },
}));

const { createKeplerPreloadApi } = await import("./preload-bridge");

test("preload exposes local backend controls without Engine lock material", async () => {
  invocations.length = 0;
  const api = createKeplerPreloadApi();
  await api.backend.restart();

  expect(invocations[0]).toEqual(["kepler:backend:restart"]);
  expect(JSON.stringify(api)).not.toContain("auth_token");
  expect(JSON.stringify(api)).not.toContain("engine.lock.json");
});
