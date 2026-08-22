import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, expect, test } from "bun:test";
import { ensureEngineRunning } from "@kosmos/ark";

const tempDirs: string[] = [];
afterEach(() => {
  for (const dir of tempDirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function createDataDir(): string {
  const dir = mkdtempSync(path.join(os.tmpdir(), "kosmos-desktop-discovery-"));
  tempDirs.push(dir);
  return dir;
}

function engineLock(pid = process.pid): Record<string, unknown> {
  return {
    format_version: 1,
    api_version: { major: 1, minor: 0, patch: 0 },
    pid,
    http_port: 4317,
    ws_port: 4318,
    auth_token: "b".repeat(64),
    started_at: "2026-07-29T00:00:00Z",
    correlation_id: "123e4567-e89b-12d3-a456-426614174000",
  };
}

test("Desktop strict discovery prefers Engine when both locks exist", async () => {
  const dataDir = createDataDir();
  writeFileSync(path.join(dataDir, "engine.lock.json"), JSON.stringify(engineLock()));
  writeFileSync(
    path.join(dataDir, "kepler.lock.json"),
    JSON.stringify({ protocol_version: { major: 1, minor: 0, patch: 0 }, pid: process.pid }),
  );

  const state = await ensureEngineRunning({
    appDataPath: dataDir,
    dataDir,
    autoLaunch: false,
    waitMs: 100,
  });
  expect(state.kind).toBe("connected");
  if (state.kind === "connected") expect(state.lock.api_version.major).toBe(1);
});

test("Desktop strict discovery fails closed with legacy lock only", async () => {
  const dataDir = createDataDir();
  writeFileSync(
    path.join(dataDir, "kepler.lock.json"),
    JSON.stringify({ protocol_version: { major: 1, minor: 0, patch: 0 }, pid: process.pid }),
  );

  const state = await ensureEngineRunning({
    appDataPath: dataDir,
    dataDir,
    autoLaunch: false,
    waitMs: 100,
  });
  expect(state.kind).toBe("absent");
  expect(JSON.stringify(state)).not.toContain("kepler.lock.json");
  expect(JSON.stringify(state)).not.toContain("protocolVersion");
});
