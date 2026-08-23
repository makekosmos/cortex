import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, expect, test } from "bun:test";
import { ensureKeplerRunning } from "@kosmos/ark";

const tempDirs: string[] = [];
afterEach(() => {
  for (const dir of tempDirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function createDataDir(): string {
  const dir = mkdtempSync(path.join(os.tmpdir(), "kosmos-desktop-discovery-"));
  tempDirs.push(dir);
  return dir;
}

function keplerLock(pid = process.pid): Record<string, unknown> {
  return {
    format_version: 1,
    protocol_version: { major: 1, minor: 0, patch: 0 },
    pid,
    ws_port: 4318,
    auth_token: "b".repeat(64),
    started_at: "2026-07-29T00:00:00Z",
    db_path: path.join(os.tmpdir(), "kosmos-test.db"),
  };
}

test("Desktop discovery connects to a live Kepler lock", async () => {
  const dataDir = createDataDir();
  writeFileSync(path.join(dataDir, "kepler.lock.json"), JSON.stringify(keplerLock()));

  const state = await ensureKeplerRunning({
    appDataPath: dataDir,
    dataDir,
    autoLaunch: false,
    waitMs: 100,
  });
  expect(state.kind).toBe("connected");
  if (state.kind === "connected") expect(state.lock.protocol_version.major).toBe(1);
});

test("Desktop discovery fails closed on an incomplete legacy lock", async () => {
  const dataDir = createDataDir();
  writeFileSync(
    path.join(dataDir, "kepler.lock.json"),
    JSON.stringify({ protocol_version: { major: 1, minor: 0, patch: 0 }, pid: process.pid }),
  );

  const state = await ensureKeplerRunning({
    appDataPath: dataDir,
    dataDir,
    autoLaunch: false,
    waitMs: 100,
  });
  expect(state.kind).toBe("launch-failed");
  expect(JSON.stringify(state)).not.toContain("protocolVersion");
});
