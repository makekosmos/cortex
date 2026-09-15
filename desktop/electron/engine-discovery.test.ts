import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, expect, test } from "../test-support/node-test.mjs";
import { ensureEngineRunning } from "@kosmos/ark";
import type { JsonRecord } from "./extension-permissions";

const tempDirs: string[] = [];
afterEach(() => {
  for (const dir of tempDirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function createDataDir(): string {
  const dir = mkdtempSync(path.join(os.tmpdir(), "kosmos-desktop-discovery-"));
  tempDirs.push(dir);
  return dir;
}

function engineLock(pid = process.pid): JsonRecord {
  return {
    format_version: 1,
    api_version: { major: 1, minor: 0, patch: 0 },
    pid,
    http_port: 4317,
    ws_port: 4318,
    auth_token: "b".repeat(64),
    started_at: "2026-07-29T00:00:00Z",
    correlation_id: "12345678-1234-4234-8234-123456789abc",
  };
}

test("Desktop discovery connects to a live Engine lock", async () => {
  const dataDir = createDataDir();
  writeFileSync(path.join(dataDir, "engine.lock.json"), JSON.stringify(engineLock()));

  const state = await ensureEngineRunning({
    appDataPath: dataDir,
    dataDir,
    autoLaunch: false,
    waitMs: 100,
  });
  expect(state.kind).toBe("connected");
  if (state.kind === "connected") expect(state.lock.api_version.major).toBe(1);
});

test("Desktop discovery fails closed on an incomplete legacy lock", async () => {
  const dataDir = createDataDir();
  writeFileSync(
    path.join(dataDir, "engine.lock.json"),
    JSON.stringify({ api_version: { major: 1, minor: 0, patch: 0 }, pid: process.pid }),
  );

  const state = await ensureEngineRunning({
    appDataPath: dataDir,
    dataDir,
    autoLaunch: false,
    waitMs: 100,
  });
  expect(state.kind).toBe("malformed");
  expect(JSON.stringify(state)).not.toContain("protocolVersion");
});
