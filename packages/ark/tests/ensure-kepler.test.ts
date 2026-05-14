// AC5 + AC6 (TS unit) для Phase 2.
//
// AC5: incompatible MAJOR → "incompatible-version" state.
// AC6: lock-файл отсутствует + autoLaunch off → "not-installed" (фолбэк апки на self-managed).

import { describe, test, expect } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";

import {
  ensureKeplerRunning,
  isPidAlive,
  readLockIfAlive,
  resolveLockPath,
} from "../src/ensure-kepler.js";

function makeTempAppData(): string {
  return fs.mkdtempSync(path.join(tmpdir(), "kosmos-ark-test-"));
}

function writeLock(appdata: string, lock: Record<string, unknown>): string {
  const lockPath = resolveLockPath(appdata);
  fs.mkdirSync(path.dirname(lockPath), { recursive: true });
  fs.writeFileSync(lockPath, JSON.stringify(lock), "utf8");
  return lockPath;
}

function cleanup(dir: string) {
  try {
    fs.rmSync(dir, { recursive: true, force: true });
  } catch {
    /* OS may hold handles temporarily on Win — лучшее усилие */
  }
}

describe("ensureKeplerRunning", () => {
  test("AC6: no lock + no exe + autoLaunch off → not-installed", async () => {
    const appdata = makeTempAppData();
    try {
      const state = await ensureKeplerRunning({
        appDataPath: appdata,
        waitMs: 100,
        autoLaunch: false,
        conventionalPaths: [],
      });
      expect(state.kind).toBe("not-installed");
    } finally {
      cleanup(appdata);
    }
  });

  test("AC6: no lock + autoLaunch on + no exe → not-installed (без spawn)", async () => {
    const appdata = makeTempAppData();
    try {
      const state = await ensureKeplerRunning({
        appDataPath: appdata,
        waitMs: 100,
        conventionalPaths: ["/nonexistent/kepler.exe"],
      });
      expect(state.kind).toBe("not-installed");
      if (state.kind === "not-installed") {
        expect(state.checkedPaths.length).toBeGreaterThan(0);
      }
    } finally {
      cleanup(appdata);
    }
  });

  test("alive PID + matching MAJOR → connected", async () => {
    const appdata = makeTempAppData();
    try {
      writeLock(appdata, {
        format_version: 1,
        protocol_version: { major: 1, minor: 0, patch: 0 },
        pid: process.pid,
        ws_port: 12345,
        auth_token: "test-token",
        started_at: "2026-05-13T00:00:00Z",
        db_path: "C:\\fake",
      });
      const state = await ensureKeplerRunning({
        appDataPath: appdata,
        autoLaunch: false,
      });
      expect(state.kind).toBe("connected");
      if (state.kind === "connected") {
        expect(state.lock.pid).toBe(process.pid);
        expect(state.lock.ws_port).toBe(12345);
      }
    } finally {
      cleanup(appdata);
    }
  });

  test("AC5: alive PID + MAJOR mismatch → incompatible-version", async () => {
    const appdata = makeTempAppData();
    try {
      writeLock(appdata, {
        format_version: 1,
        protocol_version: { major: 2, minor: 0, patch: 0 },
        pid: process.pid,
        ws_port: 12345,
        auth_token: "test-token",
        started_at: "2026-05-13T00:00:00Z",
        db_path: "C:\\fake",
      });
      const state = await ensureKeplerRunning({
        appDataPath: appdata,
        clientProtocolMajor: 1,
        autoLaunch: false,
      });
      expect(state.kind).toBe("incompatible-version");
      if (state.kind === "incompatible-version") {
        expect(state.keplerVersion.major).toBe(2);
        expect(state.clientMajor).toBe(1);
      }
    } finally {
      cleanup(appdata);
    }
  });

  test("stale lock (dead PID) + autoLaunch off → cleaned + not-installed", async () => {
    const appdata = makeTempAppData();
    try {
      const lockPath = writeLock(appdata, {
        format_version: 1,
        protocol_version: { major: 1, minor: 0, patch: 0 },
        pid: 2147483647, // impossibly high
        ws_port: 12345,
        auth_token: "test-token",
        started_at: "2026-05-13T00:00:00Z",
        db_path: "C:\\fake",
      });
      expect(fs.existsSync(lockPath)).toBe(true);

      const state = await ensureKeplerRunning({
        appDataPath: appdata,
        autoLaunch: false,
      });
      expect(state.kind).toBe("not-installed");
      // Stale lock должен быть удалён readLockIfAlive
      expect(fs.existsSync(lockPath)).toBe(false);
    } finally {
      cleanup(appdata);
    }
  });

  test("malformed lock JSON → not-installed (treated as missing)", async () => {
    const appdata = makeTempAppData();
    try {
      const lockPath = resolveLockPath(appdata);
      fs.mkdirSync(path.dirname(lockPath), { recursive: true });
      fs.writeFileSync(lockPath, "{ this is not valid JSON", "utf8");

      const state = await ensureKeplerRunning({
        appDataPath: appdata,
        autoLaunch: false,
      });
      expect(state.kind).toBe("not-installed");
    } finally {
      cleanup(appdata);
    }
  });
});

describe("readLockIfAlive", () => {
  test("returns null for missing file", () => {
    const appdata = makeTempAppData();
    try {
      expect(readLockIfAlive(resolveLockPath(appdata))).toBeNull();
    } finally {
      cleanup(appdata);
    }
  });

  test("returns null for missing required fields", () => {
    const appdata = makeTempAppData();
    try {
      const lockPath = resolveLockPath(appdata);
      fs.mkdirSync(path.dirname(lockPath), { recursive: true });
      // Missing pid / ws_port / auth_token / protocol_version
      fs.writeFileSync(lockPath, JSON.stringify({ format_version: 1 }), "utf8");
      expect(readLockIfAlive(lockPath)).toBeNull();
    } finally {
      cleanup(appdata);
    }
  });
});

describe("isPidAlive", () => {
  test("true for current PID", () => {
    expect(isPidAlive(process.pid)).toBe(true);
  });

  test("false for PID 0", () => {
    expect(isPidAlive(0)).toBe(false);
  });

  test("false for negative PID", () => {
    expect(isPidAlive(-1)).toBe(false);
  });

  test("false for impossibly high PID", () => {
    expect(isPidAlive(2147483647)).toBe(false);
  });
});
