// Phase A AC: keplerApiVersion compat check.
//
// Проверяем, что:
//   - openExtension(id) при несовместимом manifest'е открывает incompatible
//     window (не extension window).
//
// Расширение «mocha-bad» — фиктивный extension, который мы кладём в
// `.e2e/extensions-staging/<id>/` (через KOSMOS_EXTENSIONS_OVERRIDE) с
// заведомо несовместимым keplerApiVersion: "^99.0.0".

import path from "node:path";
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const e2eRoot = path.join(appRoot, ".e2e");
const userDataDir = path.join(e2eRoot, "kepler-shell-userdata-apicompat");

test.describe("extension keplerApiVersion compat", () => {
  test("AC: openExtension с несовместимым keplerApiVersion открывает incompat window, а не extension window", async () => {
    // Создаём фиктивный extension в repo dev tree — extension-host
    // resolveExtensionRoots ставит dev tree выше user. Но мы не хотим
    // мусорить в commit-ed extensions/. Вместо этого используем KOSMOS_DATA_DIR,
    // которого smoke spec не использует, чтобы переопределить user extensions
    // root через keplerDataDir().
    const dataDir = path.join(e2eRoot, "kepler-data-apicompat");
    const extDir = path.join(dataDir, "extensions", "mocha-bad");
    fs.rmSync(extDir, { recursive: true, force: true });
    fs.mkdirSync(extDir, { recursive: true });
    fs.writeFileSync(
      path.join(extDir, "manifest.json"),
      JSON.stringify(
        {
          id: "mocha-bad",
          name: "Mocha Bad",
          version: "1.0.0",
          keplerApiVersion: "^99.0.0",
          kind: "static",
          entryHtml: "index.html",
        },
        null,
        2,
      ),
    );
    fs.writeFileSync(path.join(extDir, "index.html"), "<!doctype html><title>mocha-bad</title>");

    fs.mkdirSync(userDataDir, { recursive: true });
    const app = await electron.launch({
      executablePath: electronBinary,
      cwd: appRoot,
      args: [path.join(appRoot, "dist-electron", "main.js"), `--user-data-dir=${userDataDir}`],
      env: {
        ...process.env,
        NODE_ENV: "test",
        KEPLER_SKIP_SYNC: "1",
        KOSMOS_TEST_MODE: "1",
        KOSMOS_HEADLESS: "1",
        KOSMOS_DATA_DIR: dataDir,
      },
      timeout: 20_000,
    });
    try {
      await expect
        .poll(() =>
          app.evaluate(({ ipcMain }) => {
            const handlers = (
              ipcMain as unknown as {
                _invokeHandlers?: Map<string, (...a: unknown[]) => unknown>;
              }
            )._invokeHandlers;
            return handlers?.has?.("kepler:extension:open") === true;
          }),
        )
        .toBe(true);

      // Снять список окон ДО openExtension.
      const before = await app.evaluate(({ BrowserWindow }) =>
        BrowserWindow.getAllWindows().map((w) => ({
          title: w.getTitle(),
          id: w.id,
        })),
      );

      // Дёрнуть openExtension через ipc handler (kepler:extension:open) — но
      // у Playwright нет renderer'а, в котором запущен preload; используем
      // прямой вызов из main process.
      const incompatWindowPromise = app.waitForEvent("window", { timeout: 10_000 });
      await app.evaluate(async ({ ipcMain }, id) => {
        const handlers = (
          ipcMain as unknown as {
            _invokeHandlers: Map<string, (...a: unknown[]) => unknown>;
          }
        )._invokeHandlers;
        const handler = handlers?.get?.("kepler:extension:open");
        if (!handler) throw new Error("kepler:extension:open handler not registered");
        // Fake IpcMainInvokeEvent.
        await handler({} as never, id);
      }, "mocha-bad");
      const incompatWindow = await incompatWindowPromise;
      await incompatWindow.waitForLoadState("domcontentloaded");

      const after = await app.evaluate(({ BrowserWindow }) =>
        BrowserWindow.getAllWindows().map((w) => ({
          title: w.getTitle(),
          id: w.id,
        })),
      );

      // Новое окно появилось — и его title содержит «несовместимо».
      const newWins = after.filter((w) => !before.some((b) => b.id === w.id));
      expect(newWins.length).toBeGreaterThanOrEqual(1);
      await expect
        .poll(async () => (await incompatWindow.title()).toLowerCase())
        .toContain("несовместимо");
    } finally {
      await app.close();
    }
  });
});
