import fs from "node:fs";
import { execFileSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

async function launchIsolated(
  dataDir: string,
  ambientAppDataDir?: string,
): Promise<{ app: ElectronApplication; userDataDir: string }> {
  const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-desktop-userdata-"));
  const env = {
    ...process.env,
    NODE_ENV: "test",
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_TEST_MODE: "1",
    KOSMOS_HEADLESS: "1",
    KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
    KEPLER_SKIP_SYNC: "1",
    KEPLER_BACKEND_EXE:
      process.env.KEPLER_BACKEND_EXE ??
      path.resolve(appRoot, "../../target/debug/kepler-backend.exe"),
  };
  if (ambientAppDataDir) env.APPDATA = ambientAppDataDir;
  const app = await electron.launch({
    executablePath: electronBinary,
    cwd: appRoot,
    args: [path.join(appRoot, "dist-electron", "main.js"), `--user-data-dir=${userDataDir}`],
    env,
    timeout: 20_000,
  });
  return { app, userDataDir };
}

const managerRoot = path.resolve(appRoot, "..", "manager");

async function launchManagerIsolated(
  dataDir: string,
  ambientAppDataDir?: string,
): Promise<{ app: ElectronApplication; userDataDir: string }> {
  const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-manager-userdata-"));
  const env = {
    ...process.env,
    NODE_ENV: "test",
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_TEST_MODE: "1",
    KOSMOS_HEADLESS: "1",
    KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
  };
  if (ambientAppDataDir) env.APPDATA = ambientAppDataDir;
  const app = await electron.launch({
    executablePath: electronBinary,
    cwd: managerRoot,
    args: [path.join(managerRoot, "dist-electron", "main.js"), `--user-data-dir=${userDataDir}`],
    env,
    timeout: 20_000,
  });
  return { app, userDataDir };
}

function usageSnapshot(dataDir: string): {
  clients?: Record<string, number>;
  legacy?: { connections?: number };
} | null {
  try {
// SAFETY: the test fixture or assertion setup establishes the expected contract.
    return JSON.parse(fs.readFileSync(path.join(dataDir, "protocol-usage.json"), "utf8")) as {
      clients?: Record<string, number>;
      legacy?: { connections?: number };
    };
  } catch {
    return null;
  }
}

function removeTempDir(dir: string | undefined, prefix: string): void {
  if (!dir) return;
  const resolved = path.resolve(dir);
  const tempRoot = path.resolve(os.tmpdir());
  if (
    path.dirname(resolved).toLowerCase() !== tempRoot.toLowerCase() ||
    !path.basename(resolved).startsWith(prefix)
  ) {
    return;
  }
  fs.rmSync(resolved, { recursive: true, force: true });
}

function shutdownIsolatedEngine(dataDir: string): void {
  const lockPath = path.join(dataDir, "engine.lock.json");
  if (!fs.existsSync(lockPath)) return;
  const pid = (() => {
    try {
// SAFETY: the test fixture or assertion setup establishes the expected contract.
      const parsed = JSON.parse(fs.readFileSync(lockPath, "utf8")) as { pid?: unknown };
      return isInteger(parsed.pid) ? parsed.pid : null;
    } catch {
      return null;
    }
  })();
  const exe =
    process.env.KEPLER_BACKEND_EXE ??
    path.resolve(appRoot, "../../target/debug/kepler-backend.exe");
  try {
    execFileSync(exe, ["--shutdown"], {
      env: { ...process.env, KOSMOS_DATA_DIR: dataDir, KOSMOS_LOCK_PERMISSIONS_DISABLED: "1" },
      stdio: "ignore",
      windowsHide: true,
      timeout: 10_000,
    });
  } catch {
    // Exact isolated lock is removed below; never touch another data directory.
  }
  if (pid !== null) {
    try {
      execFileSync("taskkill", ["/F", "/T", "/PID", String(pid)], {
        stdio: "ignore",
        windowsHide: true,
        timeout: 5_000,
      });
    } catch {
      // Process already exited after the graceful request.
    }
  }
}

function isInteger<T>(value: T): value is T & number {
  return typeof value === "number" && Number.isInteger(value);
}

test("Desktop and Manager share Engine v1 attribution without ambient-data access", async () => {
  test.setTimeout(120_000);
  const ambientDir = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-desktop-ambient-"));
  const sentinelPath = path.join(ambientDir, "sentinel.txt");
  fs.writeFileSync(sentinelPath, "must remain untouched\n");
  const firstDataDir = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-desktop-engine-v1-"));
  const secondDataDir = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-desktop-engine-v1-repeat-"));
  let launched: { app: ElectronApplication; userDataDir: string } | undefined;
  let manager: { app: ElectronApplication; userDataDir: string } | undefined;
  let repeated: { app: ElectronApplication; userDataDir: string } | undefined;
  try {
    launched = await launchIsolated(firstDataDir, ambientDir);
    const window =
      launched.app.windows()[0] ?? (await launched.app.waitForEvent("window", { timeout: 20_000 }));
    await window.evaluate(async () => {
      if (!window.kepler.__test) throw new Error("test bridge unavailable");
      await window.kepler.__test.waitForReady(20_000);
    });
    const result = await window.evaluate(async () => {
      let eventCount = 0;
      const commandId = `desktop-v1-${Date.now()}`;
      const eventReceived = new Promise<boolean>((resolve) => {
        const unsubscribe = window.kepler.ark.onEvent((event) => {
          if (event.event !== "commands_changed") return;
          eventCount += 1;
          unsubscribe();
          resolve(true);
        });
        void window.kepler.ark
          .request("commands.register", {
            commands: [{ id: commandId, title: "Desktop v1 test", category: "action" }],
          })
          .catch(() => resolve(false));
      });
      const subscribed = await Promise.race([
        eventReceived,
        new Promise<boolean>((resolve) => setTimeout(() => resolve(false), 10_000)),
      ]);
      await window.kepler.ark.request("commands.unregister", { ids: [commandId] }).catch(() => {});
      return { subscribed, eventCount };
    });
    expect(result.subscribed).toBe(true);
    expect(result.eventCount).toBeGreaterThan(0);

    manager = await launchManagerIsolated(firstDataDir, ambientDir);
    const managerPage =
      manager.app.windows()[0] ?? (await manager.app.waitForEvent("window", { timeout: 20_000 }));
    const health = await managerPage.evaluate(() => window.kosmosManager.getHealth());
    expect(health.ok).toBe(true);
    const info = await managerPage.evaluate(() => window.kosmosManager.getInfo());
    expect(info.ok).toBe(true);
    const dataSummary = await managerPage.evaluate(() => window.kosmosManager.getDataSummary());
    expect(dataSummary.ok).toBe(true);
    await manager.app.close();
    removeTempDir(manager.userDataDir, "kosmos-manager-userdata-");
    manager = undefined;

    await expect
      .poll(
        () => {
          const usage = usageSnapshot(firstDataDir);
          if (!usage) return null;
          return (
            Object.keys(usage.clients ?? {}).some((key) =>
              key.startsWith("api_v1:kosmos-desktop@"),
            ) &&
            Object.keys(usage.clients ?? {}).some((key) =>
              key.startsWith("api_v1:engine-manager@"),
            ) &&
            !Object.keys(usage.clients ?? {}).some((key) => key.startsWith("legacy:kosmos-")) &&
            usage.legacy?.connections === 0
          );
        },
        { timeout: 20_000 },
      )
      .toBe(true);

    shutdownIsolatedEngine(firstDataDir);
    await launched.app.close();
    removeTempDir(launched.userDataDir, "kosmos-desktop-userdata-");
    removeTempDir(firstDataDir, "kosmos-desktop-engine-v1-");
    launched = undefined;

    repeated = await launchIsolated(secondDataDir, ambientDir);
    const repeatedWindow =
      repeated.app.windows()[0] ?? (await repeated.app.waitForEvent("window", { timeout: 20_000 }));
    await repeatedWindow.evaluate(async () => {
      if (!window.kepler.__test) throw new Error("test bridge unavailable");
      await window.kepler.__test.waitForReady(20_000);
    });
    await expect
      .poll(
        () => {
          const usage = usageSnapshot(secondDataDir);
          return (
            usage &&
            Object.keys(usage.clients ?? {}).some((key) => key.startsWith("api_v1:kosmos-desktop@"))
          );
        },
        { timeout: 20_000 },
      )
      .toBe(true);
    expect(fs.readFileSync(sentinelPath, "utf8")).toBe("must remain untouched\n");
    expect(fs.existsSync(path.join(ambientDir, "engine.lock.json"))).toBe(false);
    expect(fs.existsSync(path.join(ambientDir, "kepler.lock.json"))).toBe(false);
  } finally {
    shutdownIsolatedEngine(firstDataDir);
    shutdownIsolatedEngine(secondDataDir);
    await manager?.app.close().catch(() => undefined);
    await repeated?.app.close().catch(() => undefined);
    await launched?.app.close().catch(() => undefined);
    removeTempDir(manager?.userDataDir, "kosmos-manager-userdata-");
    removeTempDir(repeated?.userDataDir, "kosmos-desktop-userdata-");
    removeTempDir(launched?.userDataDir, "kosmos-desktop-userdata-");
    removeTempDir(firstDataDir, "kosmos-desktop-engine-v1-");
    removeTempDir(secondDataDir, "kosmos-desktop-engine-v1-repeat-");
    removeTempDir(ambientDir, "kosmos-desktop-ambient-");
  }
});
