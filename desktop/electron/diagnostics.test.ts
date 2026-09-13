import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { expect, mock, test } from "../test-support/node-test.mjs";

mock.module("electron", () => ({
  app: {
    getVersion: () => "test",
    getPath: () => os.tmpdir(),
    getAppMetrics: () => [],
    getGPUFeatureStatus: () => ({}),
    getGPUInfo: async () => ({}),
  },
  BrowserWindow: { getAllWindows: () => [] },
  contentTracing: { startRecording: async () => {}, stopRecording: async () => "trace" },
  ipcMain: { handle: () => {} },
  dialog: {},
  shell: {},
}));
mock.module("./data-dir", () => ({ keplerDataDir: () => os.tmpdir() }));
mock.module("./instance", () => ({ resolveInstance: () => ({ slot: "test" }) }));
mock.module("./logging", () => ({
  keplerLog: { info() {}, warn() {}, error() {}, logsDir: () => os.tmpdir() },
}));
mock.module("./diagnostics-window-benchmark", () => ({ runWindowMoveBenchmark: async () => ({}) }));

const { copyRedactedTextFileBounded } = await import("./diagnostics");

test("protocol usage is read through bounded redaction", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "kosmos-diagnostics-"));
  const source = path.join(root, "protocol-usage.json");
  const target = path.join(root, "out.json");
  const token = "b".repeat(64);
  await writeFile(
    source,
    JSON.stringify({
      token,
      email: "alice@example.com",
      path: "C:\\Users\\alice\\secret",
      content: "private freeform",
    }) +
      "\n" +
      "x".repeat(2 * 1024 * 1024),
  );
  expect(await copyRedactedTextFileBounded(source, target)).toBe(true);
  const output = await readFile(target, "utf8");
  expect(output).not.toContain(token);
  expect(output).not.toContain("alice@example.com");
  expect(output).not.toContain("C:\\Users\\alice");
  expect(output).not.toContain("private freeform");
  expect(Buffer.byteLength(output)).toBeLessThanOrEqual(1024 * 1024);
});
