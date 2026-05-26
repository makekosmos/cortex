// Phase C AC: argv handler + install dialog window.
//
// Spawn'им Kepler с `--ext-install <path>` в argv, ждём пока main process
// поднимет install dialog window (title содержит «Установка расширения»).

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

test("AC: spawn с --ext-install <kext> открывает install dialog window", async () => {
  const runId = `${process.pid}-${Date.now()}`;
  const userDataDir = path.join(e2eRoot, `kepler-shell-userdata-argv-${runId}`);
  const dataDir = path.join(e2eRoot, `kepler-data-argv-${runId}`);
  fs.rmSync(userDataDir, { recursive: true, force: true });
  fs.rmSync(dataDir, { recursive: true, force: true });
  fs.mkdirSync(userDataDir, { recursive: true });
  fs.mkdirSync(dataDir, { recursive: true });

  const kext = path.join(e2eRoot, "argv-test.kext");
  writeZip(kext, [
    {
      name: "manifest.json",
      data: JSON.stringify({
        id: "argv-test",
        name: "Argv Test",
        version: "1.0.0",
        keplerApiVersion: "^1.0.0",
        kind: "static",
        entryHtml: "index.html",
      }),
    },
    { name: "index.html", data: "<title>argv</title>" },
  ]);

  const app = await electron.launch({
    executablePath: electronBinary,
    cwd: appRoot,
    args: [
      path.join(appRoot, "dist-electron", "main.js"),
      `--user-data-dir=${userDataDir}`,
      "--ext-install",
      kext,
    ],
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
    // Дать main process время на whenReady + open dialog + load Vue bundle.
    await new Promise((r) => setTimeout(r, 2500));

    const wins = await app.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows().map((w) => ({
        title: w.getTitle(),
        url: w.webContents.getURL(),
        visible: w.isVisible(),
      })),
    );

    // Install dialog opens via loadFile(...) с hash 'install-extension?path=...'
    // — index.html единый, отличаем окна по hash в URL.
    const hasInstallDialog = wins.some((w) => w.url.includes("install-extension"));
    expect(hasInstallDialog).toBe(true);
  } finally {
    await app.close();
  }
});
