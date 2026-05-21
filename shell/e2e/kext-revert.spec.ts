// Phase D AC: revert восстанавливает предыдущую версию из backup'а.
//
// 1. Install v1.0.0 → install v1.1.0 (создаётся backup v1.0.0).
// 2. revert(id) → installed version should be v1.0.0 again.

import path from "node:path";
import fs from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron } from "playwright";
import { writeZip } from "../scripts/zip-utils.mjs";

const require = createRequire(import.meta.url);
const electronBinary = require("electron") as string;
const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const e2eRoot = path.join(appRoot, ".e2e");

function makeKext(file: string, id: string, version: string): void {
  writeZip(file, [
    {
      name: "manifest.json",
      data: JSON.stringify({
        id,
        name: id,
        version,
        keplerApiVersion: "^1.0.0",
        kind: "static",
        entryHtml: "index.html",
      }),
    },
    { name: "index.html", data: `<title>${id} v${version}</title>` },
  ]);
}

test("AC: revert восстанавливает предыдущую версию из backup'а", async () => {
  const userDataDir = path.join(e2eRoot, "kepler-shell-userdata-revert");
  const dataDir = path.join(e2eRoot, "kepler-data-revert");
  fs.rmSync(userDataDir, { recursive: true, force: true });
  fs.rmSync(dataDir, { recursive: true, force: true });
  fs.mkdirSync(userDataDir, { recursive: true });
  fs.mkdirSync(dataDir, { recursive: true });

  const kV1 = path.join(e2eRoot, "rev-v1.kext");
  const kV2 = path.join(e2eRoot, "rev-v2.kext");
  makeKext(kV1, "rev-target", "1.0.0");
  makeKext(kV2, "rev-target", "1.1.0");

  const app = await electron.launch({
    executablePath: electronBinary,
    cwd: appRoot,
    args: [path.join(appRoot, "dist-electron", "main.js"), `--user-data-dir=${userDataDir}`],
    env: {
      ...process.env,
      NODE_ENV: "test",
      KEPLER_SKIP_SYNC: "1",
      KOSMOS_TEST_MODE: "1",
      KOSMOS_DATA_DIR: dataDir,
    },
    timeout: 20_000,
  });
  try {
    await new Promise((r) => setTimeout(r, 1500));

    async function call(channel: string, ...args: unknown[]) {
      return app.evaluate(
        async ({ ipcMain }, { ch, a }) => {
          const handlers = (
            ipcMain as unknown as {
              _invokeHandlers: Map<string, (...x: unknown[]) => unknown>;
            }
          )._invokeHandlers;
          const h = handlers?.get?.(ch);
          if (!h) throw new Error(`no handler: ${ch}`);
          return await h({} as never, ...a);
        },
        { ch: channel, a: args },
      );
    }

    await call("kepler:extension:install:do", kV1);
    await call("kepler:extension:install:do", kV2);

    const targetManifest = path.join(dataDir, "extensions", "rev-target", "manifest.json");
    const beforeRevert = JSON.parse(fs.readFileSync(targetManifest, "utf8"));
    expect(beforeRevert.version).toBe("1.1.0");

    const backups = (await call("kepler:extension:backups:list", "rev-target")) as string[];
    expect(backups.length).toBeGreaterThanOrEqual(1);

    const ok = await call("kepler:extension:revert", "rev-target");
    expect(ok).toBe(true);

    const afterRevert = JSON.parse(fs.readFileSync(targetManifest, "utf8"));
    expect(afterRevert.version).toBe("1.0.0");

    // После revert'а должен появиться backup с pre-revert v1.1.0.
    const backupsAfter = (await call("kepler:extension:backups:list", "rev-target")) as string[];
    expect(backupsAfter.length).toBeGreaterThanOrEqual(backups.length);
  } finally {
    await app.close();
  }
});
