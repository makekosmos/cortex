import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, expect, mock, test } from "../test-support/node-test.mjs";

mock.module("electron", () => ({
  app: { getPath: () => "C:\\Kosmos-test" },
  ipcRenderer: {
    invoke: async () => undefined,
    on: () => {},
    removeListener: () => {},
    send: () => {},
  },
}));
const { isBackendLockProcessAlive } = await import("./main-backend-process");

const tempDirs: string[] = [];
afterEach(() => {
  for (const dir of tempDirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

test("supervisor accepts only a live Engine lock PID", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "kosmos-desktop-engine-lock-"));
  tempDirs.push(dir);
  const lockPath = path.join(dir, "engine.lock.json");
  writeFileSync(lockPath, JSON.stringify({ pid: process.pid }));
  expect(isBackendLockProcessAlive(lockPath)).toBe(true);

  writeFileSync(lockPath, JSON.stringify({ pid: 999999999 }));
  expect(isBackendLockProcessAlive(lockPath)).toBe(false);
});

test("production Desktop spawn does not own Usage Tracker policy", () => {
  const source = readFileSync(new URL("./main-backend-process.ts", import.meta.url), "utf8");
  expect(source).not.toContain("KEPLER_USAGE_TRACKER");
  expect(source).not.toContain("isUsageTrackerEnabled");
});
