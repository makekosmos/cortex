import fs from "node:fs";
import path from "node:path";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";
import { DatabaseSync } from "node:sqlite";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";

const managerRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const engineBinary = path.resolve(
  managerRoot,
  "..",
  "..",
  "target",
  "debug",
  "kepler-backend.exe",
);
const runRoot = path.join(
  managerRoot,
  ".e2e",
  "engine-manager",
  `${process.pid}-${Date.now()}`,
);
const dataDir = path.join(runRoot, "data");
const lockPath = path.join(dataDir, "engine.lock.json");

type EngineLock = {
  pid: number;
  http_port: number;
  api_version: { major: number; minor: number; patch: number };
};

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitFor<T>(
  read: () => T | null,
  timeoutMs = 30_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = read();
    if (value !== null) return value;
    await delay(200);
  }
  throw new Error(`Timed out after ${timeoutMs}ms`);
}

function readLock(): EngineLock | null {
  try {
    // SAFETY: the fixture lock file is written by the Engine launcher with this schema.
    const parsed = JSON.parse(fs.readFileSync(lockPath, "utf8")) as EngineLock;
    return parsed.pid > 0 && parsed.http_port > 0 ? parsed : null;
  } catch {
    return null;
  }
}

function startEngine(
  usageTrackerOverride: "0" | "1" | undefined,
): ChildProcess {
  if (!fs.existsSync(engineBinary))
    throw new Error(`Missing Engine binary: ${engineBinary}`);
  fs.mkdirSync(dataDir, { recursive: true });
  const env = {
    ...process.env,
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
    KOSMOS_LOCAL_STT_DIR: dataDir,
    KOSMOS_TEST_MODE: "1",
    KOSMOS_TEST_GROQ_API_KEY: "fixture-key-not-user-data",
    KEPLER_SKIP_SYNC: "1",
    RUST_LOG: "error",
  };
  if (usageTrackerOverride) env.KEPLER_USAGE_TRACKER = usageTrackerOverride;
  else delete env.KEPLER_USAGE_TRACKER;
  return spawn(engineBinary, [], {
    cwd: managerRoot,
    stdio: "pipe",
    env,
  });
}

async function launchManager(
  slot: string,
  entry = path.join(managerRoot, "dist-electron", "main.js"),
): Promise<ElectronApplication> {
  const userDataDir = path.join(runRoot, slot, "userdata");
  fs.mkdirSync(userDataDir, { recursive: true });
  return electron.launch({
    executablePath: electronBinary,
    cwd: managerRoot,
    args: [`--user-data-dir=${userDataDir}`, entry],
    env: {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      NODE_ENV: "test",
    },
    timeout: 30_000,
  });
}

function engineIsAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

type UsageRowCounts = {
  tracked_apps: number;
  usage_sessions: number;
  usage_events: number;
  usage_sync_versions: number;
};
function readUsageRowCounts(): UsageRowCounts {
  const database = new DatabaseSync(path.join(dataDir, "ark.db"), {
    readOnly: true,
  });
  try {
    const count = (table: string) => {
      // SAFETY: each query selects a single numeric count column.
      const row = database
        .prepare(`SELECT COUNT(*) AS count FROM ${table}`)
        .get() as {
        count: number;
      };
      return Number(row.count);
    };
    // SAFETY: the aggregate query selects a single numeric count column.
    const syncRows = database
      .prepare(
        "SELECT COUNT(*) AS count FROM usage_sync_versions WHERE entity_type IN ('usage_session', 'usage_event', 'usage_day')",
      )
      // SAFETY: the aggregate query selects a single numeric count column.
      // SAFETY: the aggregate query selects a single numeric count column.
      .get() as { count: number };
    return {
      tracked_apps: count("tracked_apps"),
      usage_sessions: count("usage_sessions"),
      usage_events: count("usage_events"),
      usage_sync_versions: Number(syncRows.count),
    };
  } finally {
    database.close();
  }
}

test.describe("Engine Manager isolated lifecycle", () => {
  test("migrates legacy Usage Tracker policy before boot and applies Manager changes on restart", async () => {
    fs.rmSync(runRoot, { recursive: true, force: true });
    fs.mkdirSync(dataDir, { recursive: true });
    fs.writeFileSync(
      path.join(dataDir, "kepler-shell-settings.json"),
      JSON.stringify({
        usageTrackerEnabled: false,
        hiddenCommandIds: ["kepler:focus-session"],
      }),
    );
    const legacySettings = fs.readFileSync(
      path.join(dataDir, "kepler-shell-settings.json"),
      "utf8",
    );
    let engine: ChildProcess | undefined;
    let manager: ElectronApplication | undefined;
    try {
      engine = startEngine(undefined);
      await waitFor(readLock);
      manager = await launchManager("usage-migration");
      const page = await manager.firstWindow();
      const initial = await page.evaluate(() =>
        window.kosmosManager.getEngineSettings(),
      );
      expect(initial).toMatchObject({
        ok: true,
        data: { usage_tracker: { enabled: false } },
      });
      const autostart = await page.evaluate(() =>
        window.kosmosManager.getAutostart(),
      );
      expect(autostart).toMatchObject({
        ok: true,
        data: { available: false, enabled: false },
      });
      const diagnostics = await page.evaluate(() =>
        window.kosmosManager.getDiagnosticsSnapshot(),
      );
      expect(diagnostics.ok).toBe(true);
      if (diagnostics.ok) {
        // SAFETY: diagnostics component entries use the documented component shape.
        const usage = diagnostics.data.components.find(
          (component) => component.name === "usage_tracker",
          // SAFETY: diagnostics component entries use the documented component shape.
          // SAFETY: diagnostics component entries use the documented component shape.
        ) as
          | {
              state?: string;
              details?: {
                configured_enabled?: boolean;
                running?: boolean;
                status?: string;
              };
            }
          | undefined;
        expect(usage?.state).toBe("disabled");
        expect(usage?.details).toMatchObject({
          configured_enabled: false,
          running: false,
          status: "disabled",
        });
      }
      expect(readUsageRowCounts()).toEqual({
        tracked_apps: 0,
        usage_sessions: 0,
        usage_events: 0,
        usage_sync_versions: 0,
      });
      expect(
        fs.readFileSync(
          path.join(dataDir, "kepler-shell-settings.json"),
          "utf8",
        ),
      ).toBe(legacySettings);
      // SAFETY: this fixture settings file is written with the usage_tracker shape.
      const migrated = JSON.parse(
        fs.readFileSync(
          path.join(dataDir, "engine-manager-settings.json"),
          "utf8",
        ),
        // SAFETY: this fixture settings file is written with the usage_tracker shape.
        // SAFETY: this fixture settings file is written with the usage_tracker shape.
      ) as { usage_tracker?: { enabled?: boolean } };
      expect(migrated.usage_tracker?.enabled).toBe(false);

      const updated = await page.evaluate(() =>
        window.kosmosManager.setUsageTracker({ enabled: true }),
      );
      expect(updated).toMatchObject({
        ok: true,
        data: { usage_tracker: { enabled: true } },
      });
      const stillDisabled = await page.evaluate(() =>
        window.kosmosManager.getDiagnosticsSnapshot(),
      );
      expect(stillDisabled.ok).toBe(true);
      if (stillDisabled.ok) {
        const usage = stillDisabled.data.components.find(
          (component) => component.name === "usage_tracker",
        );
        expect(usage?.state).toBe("disabled");
      }

      await manager.close();
      manager = undefined;
      spawnSync(engineBinary, ["--shutdown"], {
        cwd: managerRoot,
        env: {
          ...process.env,
          KOSMOS_DATA_DIR: dataDir,
          KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
        },
        stdio: "ignore",
      });
      await delay(500);
      if (engineIsAlive(engine.pid ?? -1)) engine.kill();
      engine = startEngine(undefined);
      await waitFor(readLock);
      manager = await launchManager("usage-migration-restart");
      const restarted = await manager
        .firstWindow()
        .then((next) =>
          next.evaluate(() => window.kosmosManager.getEngineSettings()),
        );
      expect(restarted).toMatchObject({
        ok: true,
        data: { usage_tracker: { enabled: true } },
      });
      expect(
        fs.readFileSync(
          path.join(dataDir, "kepler-shell-settings.json"),
          "utf8",
        ),
      ).toBe(legacySettings);
      const running = await manager
        .firstWindow()
        .then((next) =>
          next.evaluate(() => window.kosmosManager.getDiagnosticsSnapshot()),
        );
      expect(running.ok).toBe(true);
      if (running.ok) {
        // SAFETY: diagnostics component entries use the documented component shape.
        const usage = running.data.components.find(
          (component) => component.name === "usage_tracker",
          // SAFETY: diagnostics component entries use the documented component shape.
          // SAFETY: diagnostics component entries use the documented component shape.
        ) as { details?: { configured_enabled?: boolean } } | undefined;
        expect(usage?.details?.configured_enabled).toBe(true);
      }
    } finally {
      await manager?.close().catch(() => undefined);
      if (engine) {
        spawnSync(engineBinary, ["--shutdown"], {
          cwd: managerRoot,
          env: {
            ...process.env,
            KOSMOS_DATA_DIR: dataDir,
            KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
          },
          stdio: "ignore",
        });
        await delay(500);
        if (engineIsAlive(engine.pid ?? -1)) engine.kill();
      }
      fs.rmSync(runRoot, { recursive: true, force: true });
    }
  });

  test("opens the migrated dictation configuration without changing retained assets", async () => {
    fs.rmSync(runRoot, { recursive: true, force: true });
    const fixtureId = "11111111-1111-4111-8111-111111111111";
    const config = JSON.stringify({
      hotkey: "Ctrl+Alt+D",
      language: "ru",
      triggerMode: "toggle",
      injectMode: "clipboard_only",
      provider: "groq",
      providerEnabled: true,
      model: "whisper-large-v3-turbo",
      networkProfile: { kind: "system" },
    });
    const pendingMeta = JSON.stringify({
      uuid: fixtureId,
      created_at: new Date().toISOString(),
      attempts: 1,
      last_error: "fixture",
      opts: {
        language: "ru",
        prompt: "",
        inject_mode: "clipboard_only",
        model: "whisper-large-v3-turbo",
        prev_hwnd: null,
      },
      duration_sec: 1,
      wav_bytes: 3,
    });
    const backupPath = path.join(dataDir, "dictation-config.json.bak");
    const pendingPath = path.join(dataDir, "dictation", "pending");
    const markerPath = path.join(
      dataDir,
      "models",
      "dictation",
      "fixture.marker",
    );
    fs.mkdirSync(path.dirname(markerPath), { recursive: true });
    fs.mkdirSync(pendingPath, { recursive: true });
    fs.writeFileSync(path.join(dataDir, "dictation-config.json"), config);
    fs.writeFileSync(backupPath, config);
    fs.writeFileSync(path.join(pendingPath, `${fixtureId}.json`), pendingMeta);
    fs.writeFileSync(path.join(pendingPath, `${fixtureId}.wav`), "wav");
    fs.writeFileSync(markerPath, "asset-marker");
    let engine: ChildProcess | undefined;
    let manager: ElectronApplication | undefined;
    try {
      engine = startEngine("0");
      await waitFor(readLock);
      await expect(
        launchManager(
          "dictation-migration-failed",
          path.join(managerRoot, "dist-electron", "missing-main.js"),
        ),
      ).rejects.toThrow();
      expect(
        fs.readFileSync(path.join(dataDir, "dictation-config.json"), "utf8"),
      ).toBe(config);
      expect(fs.readFileSync(backupPath, "utf8")).toBe(config);
      expect(
        fs.readFileSync(path.join(pendingPath, `${fixtureId}.json`), "utf8"),
      ).toBe(pendingMeta);
      expect(
        fs.readFileSync(path.join(pendingPath, `${fixtureId}.wav`), "utf8"),
      ).toBe("wav");
      expect(fs.readFileSync(markerPath, "utf8")).toBe("asset-marker");
      manager = await launchManager("dictation-migration");
      await manager.close();
      manager = await launchManager("dictation-migration-reload");
      const page = await manager.firstWindow();
      await page.getByRole("button", { name: "Диктовка и AI" }).click();
      await expect(page.locator("[aria-label='Диктовка и AI']")).toBeVisible();
      const before = await page.evaluate(() =>
        window.kosmosManager.getDictationConfig(),
      );
      expect(before).toMatchObject({
        ok: true,
        data: {
          hasApiKey: true,
          config: { injectMode: "clipboard_only", hotkey: "Ctrl+Alt+D" },
        },
      });
      expect(fs.readFileSync(backupPath, "utf8")).toBe(config);
      expect(
        fs.readFileSync(path.join(pendingPath, `${fixtureId}.json`), "utf8"),
      ).toBe(pendingMeta);
      expect(
        fs.readFileSync(path.join(pendingPath, `${fixtureId}.wav`), "utf8"),
      ).toBe("wav");
      expect(fs.readFileSync(markerPath, "utf8")).toBe("asset-marker");
      const updated = await page.evaluate(() =>
        window.kosmosManager.updateDictationConfig({ language: "en" }),
      );
      expect(updated).toMatchObject({
        ok: true,
        data: { config: { language: "en", injectMode: "clipboard_only" } },
      });
      const cleared = await page.evaluate(() =>
        window.kosmosManager.clearDictationApiKey(),
      );
      expect(cleared).toEqual({ ok: true, data: { cleared: true } });
      const windows = await manager.evaluate(({ BrowserWindow }) =>
        BrowserWindow.getAllWindows().map((window) => window.isVisible()),
      );
      expect(windows).toEqual([false]);
      expect(
        fs.readFileSync(path.join(pendingPath, `${fixtureId}.json`), "utf8"),
      ).toBe(pendingMeta);
      expect(
        fs.readFileSync(path.join(pendingPath, `${fixtureId}.wav`), "utf8"),
      ).toBe("wav");
      expect(fs.readFileSync(markerPath, "utf8")).toBe("asset-marker");
    } finally {
      await manager?.close().catch(() => undefined);
      if (engine) {
        spawnSync(engineBinary, ["--shutdown"], {
          cwd: managerRoot,
          env: {
            ...process.env,
            KOSMOS_DATA_DIR: dataDir,
            KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
          },
          stdio: "ignore",
        });
        await delay(500);
        if (engineIsAlive(engine.pid ?? -1)) engine.kill();
      }
      fs.rmSync(runRoot, { recursive: true, force: true });
    }
  });

  test("attaches to v1 Engine, exits independently, and reattaches", async () => {
    fs.rmSync(runRoot, { recursive: true, force: true });
    fs.mkdirSync(dataDir, { recursive: true });
    let engine: ChildProcess | undefined;
    let manager: ElectronApplication | undefined;
    let reopened: ElectronApplication | undefined;
    try {
      engine = startEngine("0");
      const lock = await waitFor(readLock);
      expect(lock.api_version.major).toBe(1);
      expect(engineIsAlive(lock.pid)).toBe(true);

      manager = await launchManager("first");
      const page = await manager.firstWindow();
      await expect(
        page.locator("[aria-label='Разделы менеджера']"),
      ).toBeVisible();
      const windows = await manager.evaluate(({ BrowserWindow }) =>
        BrowserWindow.getAllWindows().map((window) => ({
          visible: window.isVisible(),
          focused: window.isFocused(),
        })),
      );
      expect(windows).toEqual([{ visible: false, focused: false }]);
      const capabilityKeys = await page.evaluate(() =>
        Object.keys(window.kosmosManager).sort(),
      );
      expect(capabilityKeys).not.toContain("auth_token");
      expect(capabilityKeys).not.toContain("lock");
      expect(capabilityKeys).not.toContain("http");
      expect(capabilityKeys).toContain("getHealth");

      const health = await page.evaluate(() =>
        window.kosmosManager.getHealth(),
      );
      expect(health.ok).toBe(true);
      const info = await page.evaluate(() => window.kosmosManager.getInfo());
      expect(info.ok).toBe(true);
      const cancelled = await page.evaluate(() =>
        window.kosmosManager.saveSupportBundle(),
      );
      expect(cancelled.ok).toBe(true);
      if (cancelled.ok)
        expect(cancelled.data).toEqual({ saved: false, cancelled: true });

      await manager.close();
      manager = undefined;
      await delay(500);
      expect(engineIsAlive(lock.pid)).toBe(true);
      expect(readLock()).toEqual(lock);

      reopened = await launchManager("reopen");
      const reopenedPage = await reopened.firstWindow();
      const reopenedHealth = await reopenedPage.evaluate(() =>
        window.kosmosManager.getHealth(),
      );
      expect(reopenedHealth.ok).toBe(true);
      expect(readLock()).toEqual(lock);

      const usagePath = path.join(dataDir, "protocol-usage.json");
      const usage = await waitFor(() => {
        try {
          // SAFETY: protocol usage fixture is written with these aggregate fields.
          const parsed = JSON.parse(fs.readFileSync(usagePath, "utf8")) as {
            api_v1?: { connections?: number };
            legacy?: { connections?: number };
            clients?: { [key: string]: number | string | boolean | null };
          };
          return parsed.api_v1?.connections &&
            Object.keys(parsed.clients ?? {}).some((key) =>
              key.startsWith("api_v1:engine-manager@"),
            )
            ? parsed
            : null;
        } catch {
          return null;
        }
      });
      expect(usage.api_v1?.connections).toBeGreaterThan(0);
      expect(usage.legacy?.connections ?? 0).toBe(0);
    } finally {
      await reopened?.close().catch(() => undefined);
      await manager?.close().catch(() => undefined);
      if (engine) {
        spawnSync(engineBinary, ["--shutdown"], {
          cwd: managerRoot,
          env: {
            ...process.env,
            KOSMOS_DATA_DIR: dataDir,
            KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
          },
          stdio: "ignore",
        });
        await delay(500);
        if (engineIsAlive(engine.pid ?? -1)) engine.kill();
      }
      fs.rmSync(runRoot, { recursive: true, force: true });
    }
  });
});
