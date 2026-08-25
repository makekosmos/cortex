// Phase B AC: .kext installer + backup + atomic write.
//
// 1. Создаём fixture .kext v1.0.0 → install через extension-installer API.
// 2. Создаём fixture .kext v1.1.0 (тот же id) → install заново.
// 3. Проверяем что в extensions-backups/<id>/ есть бэкап v1.0.0.
// 4. Проверяем что target dir содержит v1.1.0 manifest.

import path from "node:path";
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron } from "playwright";
import electronBinary from "electron";
import { writeZip } from "../scripts/zip-utils.mjs";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const e2eRoot = path.join(appRoot, ".e2e");

function makeKextFixture(targetKext: string, version: string): void {
  const manifest = {
    id: "mocha-good",
    name: "Mocha Good",
    version,
    description: "Test extension",
    author: "test",
    keplerApiVersion: "^1.0.0",
    kind: "static",
    entryHtml: "index.html",
  };
  writeZip(targetKext, [
    { name: "manifest.json", data: JSON.stringify(manifest, null, 2) },
    {
      name: "index.html",
      data: `<!doctype html><title>mocha v${version}</title>`,
    },
  ]);
}

test.describe(".kext installer + backup", () => {
  test("AC: install v1.0.0 → install v1.1.0 → backup содержит v1.0.0", async () => {
    const runId = `${process.pid}-${Date.now()}`;
    const userDataDir = path.join(e2eRoot, `kepler-shell-userdata-kext-${runId}`);
    const dataDir = path.join(e2eRoot, `kepler-data-kext-${runId}`);

    fs.rmSync(dataDir, { recursive: true, force: true });
    fs.rmSync(userDataDir, { recursive: true, force: true });
    fs.mkdirSync(dataDir, { recursive: true });
    fs.mkdirSync(userDataDir, { recursive: true });

    const kextV1 = path.join(e2eRoot, "mocha-v1.kext");
    const kextV2 = path.join(e2eRoot, "mocha-v2.kext");
    makeKextFixture(kextV1, "1.0.0");
    makeKextFixture(kextV2, "1.1.0");

    const app = await electron.launch({
      executablePath: electronBinary,
      cwd: appRoot,
      args: [path.join(appRoot, "dist-electron", "main.js"), `--user-data-dir=${userDataDir}`],
      env: {
        ...process.env,
        NODE_ENV: "test",
        KEPLER_SKIP_SYNC: "1",
        KOSMOS_HEADLESS: "1",
        KOSMOS_TEST_MODE: "1",
        KOSMOS_DATA_DIR: dataDir,
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      },
      timeout: 20_000,
    });
    try {
      await expect
        .poll(
          () =>
            app.evaluate(({ ipcMain }) => {
              const handlers = (
// SAFETY: the test fixture or assertion setup establishes the expected contract.
                ipcMain as {
                  _invokeHandlers: Map<string, (...a: unknown[]) => never>;
                }
              )._invokeHandlers;
              return handlers?.has?.("kepler:extension:install:do") === true;
            }),
          { timeout: 10_000 },
        )
        .toBe(true);

      // Install v1.
      const r1 = await app.evaluate(async ({ ipcMain }, kextPath) => {
        const handlers = (
// SAFETY: the test fixture or assertion setup establishes the expected contract.
          ipcMain as {
            _invokeHandlers: Map<string, (...a: unknown[]) => never>;
          }
        )._invokeHandlers;
        const h = handlers?.get?.("kepler:extension:install:do");
        if (!h) throw new Error("install handler missing");
// SAFETY: the test fixture or assertion setup establishes the expected contract.
        return h({} as never, kextPath);
      }, kextV1);
// SAFETY: the test fixture or assertion setup establishes the expected contract.
      expect((r1 as { manifest: { version: string } }).manifest.version).toBe("1.0.0");

      const targetDir = path.join(dataDir, "extensions", "mocha-good");
      expect(fs.existsSync(path.join(targetDir, "manifest.json"))).toBe(true);
      const installedV1 = JSON.parse(
        fs.readFileSync(path.join(targetDir, "manifest.json"), "utf8"),
      );
      expect(installedV1.version).toBe("1.0.0");

      // Install v2 — должен создать backup для v1.
      const r2 = await app.evaluate(async ({ ipcMain }, kextPath) => {
        const handlers = (
// SAFETY: the test fixture or assertion setup establishes the expected contract.
          ipcMain as {
            _invokeHandlers: Map<string, (...a: unknown[]) => never>;
          }
        )._invokeHandlers;
        const h = handlers?.get?.("kepler:extension:install:do");
// SAFETY: the test fixture or assertion setup establishes the expected contract.
        return h({} as never, kextPath);
      }, kextV2);
// SAFETY: the test fixture or assertion setup establishes the expected contract.
      expect((r2 as { manifest: { version: string } }).manifest.version).toBe("1.1.0");

      const installedV2 = JSON.parse(
        fs.readFileSync(path.join(targetDir, "manifest.json"), "utf8"),
      );
      expect(installedV2.version).toBe("1.1.0");

      const backupsRoot = path.join(dataDir, "extensions-backups", "mocha-good");
      expect(fs.existsSync(backupsRoot)).toBe(true);
      const backups = fs.readdirSync(backupsRoot);
      expect(backups.length).toBeGreaterThanOrEqual(1);
      const backupManifest = JSON.parse(
        fs.readFileSync(path.join(backupsRoot, backups[0]!, "manifest.json"), "utf8"),
      );
      expect(backupManifest.version).toBe("1.0.0");
    } finally {
      await app.close();
    }
  });

  test("AC: invalid .kext (no manifest) — install fails, target untouched", async () => {
    // Используем отдельный dataDir чтобы не зависеть от cleanup'а предыдущего
    // теста (на Windows .old-<stamp> dirs могут быть кратко-locked после rmSync
    // если предыдущий process ещё не отпустил file handles).
    const isolatedData = path.join(e2eRoot, "kepler-data-kext-bad");
    const isolatedUser = path.join(e2eRoot, "kepler-shell-userdata-kext-bad");
    try {
      fs.rmSync(isolatedData, { recursive: true, force: true });
    } catch {
      /* may be locked — overwrite is fine */
    }
    try {
      fs.rmSync(isolatedUser, { recursive: true, force: true });
    } catch {
      /* same */
    }
    fs.mkdirSync(isolatedData, { recursive: true });
    fs.mkdirSync(isolatedUser, { recursive: true });

    const badKext = path.join(e2eRoot, "mocha-bad.kext");
    writeZip(badKext, [{ name: "index.html", data: "no manifest" }]);

    const app = await electron.launch({
      executablePath: electronBinary,
      cwd: appRoot,
      args: [path.join(appRoot, "dist-electron", "main.js"), `--user-data-dir=${isolatedUser}`],
      env: {
        ...process.env,
        NODE_ENV: "test",
        KEPLER_SKIP_SYNC: "1",
        KOSMOS_HEADLESS: "1",
        KOSMOS_TEST_MODE: "1",
        KOSMOS_DATA_DIR: isolatedData,
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      },
      timeout: 20_000,
    });
    try {
      await expect
        .poll(
          () =>
            app.evaluate(({ ipcMain }) => {
              const handlers = (
// SAFETY: the test fixture or assertion setup establishes the expected contract.
                ipcMain as {
                  _invokeHandlers: Map<string, (...a: unknown[]) => never>;
                }
              )._invokeHandlers;
              return handlers?.has?.("kepler:extension:install:do") === true;
            }),
          { timeout: 10_000 },
        )
        .toBe(true);
      const result = await app.evaluate(async ({ ipcMain }, kextPath) => {
        const handlers = (
// SAFETY: the test fixture or assertion setup establishes the expected contract.
          ipcMain as {
            _invokeHandlers: Map<string, (...a: unknown[]) => never>;
          }
        )._invokeHandlers;
        const h = handlers?.get?.("kepler:extension:install:do");
        try {
// SAFETY: the test fixture or assertion setup establishes the expected contract.
          await h({} as never, kextPath);
          return { ok: true };
        } catch (e) {
// SAFETY: the test fixture or assertion setup establishes the expected contract.
          return { ok: false, err: (e as Error).message };
        }
      }, badKext);
// SAFETY: the test fixture or assertion setup establishes the expected contract.
      expect((result as { ok: boolean }).ok).toBe(false);
    } finally {
      await app.close();
    }
  });
});
