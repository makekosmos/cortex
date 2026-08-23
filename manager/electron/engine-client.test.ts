import { afterEach, beforeEach, expect, mock, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

let dataDir = "";

mock.module("electron", () => ({
  app: { getPath: () => "C:\\Kosmos-test", getVersion: () => "9.8.7" },
}));
const { connectEngine } = await import("./engine-client");

beforeEach(() => {
  dataDir = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-manager-engine-"));
  process.env.KOSMOS_DATA_DIR = dataDir;
  fs.writeFileSync(
    path.join(dataDir, "engine.lock.json"),
    JSON.stringify({
      format_version: 1,
      api_version: { major: 2, minor: 0, patch: 0 },
      pid: 1,
      http_port: 12345,
      auth_token: "test-token",
    }),
  );
});

afterEach(() => {
  delete process.env.KOSMOS_DATA_DIR;
  fs.rmSync(dataDir, { recursive: true, force: true });
});

test("Manager maps strict Engine incompatible state to safe Russian result", async () => {
  await expect(connectEngine()).resolves.toEqual({
    ok: false,
    code: "incompatible_api",
    message: "Версия Engine несовместима. Обновите Kosmos.",
  });
});
