import { afterEach, beforeEach, expect, mock, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

let dataDir = "";

mock.module("electron", () => ({
  app: { getPath: () => "C:\\Kosmos-test", getVersion: () => "9.8.7" },
}));
const { connectEngine, rpc } = await import("./engine-client");

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

test("Manager preserves the Engine error returned by an RPC", async () => {
  fs.writeFileSync(
    path.join(dataDir, "engine.lock.json"),
    JSON.stringify({
      format_version: 1,
      api_version: { major: 1, minor: 0, patch: 0 },
      pid: 1,
      http_port: 12345,
      auth_token: "test-token",
    }),
  );
  const previousFetch = globalThis.fetch;
  // SAFETY: the mock implements the fetch call shape exercised by rpc().
  globalThis.fetch = (() =>
    Promise.resolve(
      new Response(JSON.stringify({ ok: false, error: "Сессия истекла" })),
    )) as typeof fetch;
  try {
    await expect(rpc("integrations.sync_now")).resolves.toEqual({
      ok: false,
      code: "engine",
      message: "Сессия истекла",
    });
  } finally {
    globalThis.fetch = previousFetch;
  }
});
