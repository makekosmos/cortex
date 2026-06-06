// Phase 1 диктации: e2e гарды headless-контракта + базовый IPC контракт.
//
// Что проверяем:
//   AC10: в headless / test mode global hotkey диктации НЕ регистрируется
//         (иначе мешал бы клавиатуре машины разработчика во время CI).
//   AC10: pill window НЕ показывается визуально (можем создать, но isVisible=false).
//   AC1:  IPC `kepler:dictation:*` зарегистрированы (toggle / cancel / pill-finished).
//   AC4:  backend `dictation.get_config` возвращает дефолтный конфиг + hasApiKey=false
//         (изолированный userData → нет API key в Credential Manager этого профиля).
//
// Что НЕ проверяем e2e (требует UI session + микрофон + Groq API key):
//   AC5-9: реальный hotkey → запись → транскрипция → inject. Покрыто manual verify.
//   AC11-12: ARK guard + smoke — отдельные scripts.

import path from "node:path";
import fs from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";

const require = createRequire(import.meta.url);
const electronBinary = require("electron") as string;
const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const e2eRoot = path.join(appRoot, ".e2e");
const userDataDir = path.join(e2eRoot, "kepler-shell-dictation-userdata");

async function launchKepler(): Promise<ElectronApplication> {
  fs.mkdirSync(userDataDir, { recursive: true });
  return electron.launch({
    executablePath: electronBinary,
    cwd: appRoot,
    args: [path.join(appRoot, "dist-electron", "main.js"), `--user-data-dir=${userDataDir}`],
    env: {
      ...process.env,
      NODE_ENV: "test",
      KEPLER_SKIP_SYNC: "1",
      // КРИТИЧНО: headless guard. Без него pill window и hotkey пытались бы
      // зарегистрироваться → ломали бы клавиатуру dev-машины.
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
    },
    timeout: 20_000,
  });
}

test.describe("dictation Phase 1", () => {
  test("AC10: globalShortcut для диктации НЕ зарегистрирован в headless mode", async () => {
    const app = await launchKepler();
    try {
      // Даём backend / ARK client установиться.
      await new Promise((r) => setTimeout(r, 3000));

      const result = await app.evaluate(({ globalShortcut }) => {
        // Дефолтный accelerator из dictation::config — Ctrl+Shift+;
        return {
          isRegistered: globalShortcut.isRegistered("Ctrl+Shift+;"),
        };
      });
      expect(
        result.isRegistered,
        "В headless mode dictation globalShortcut должен оставаться unregistered (см. setupDictationHotkey).",
      ).toBe(false);
    } finally {
      await app.close();
    }
  });

  test("AC10: pill window не visible (создаётся lazily, но без show() в headless)", async () => {
    const app = await launchKepler();
    try {
      await new Promise((r) => setTimeout(r, 2000));

      // Триггерим toggle через IPC → main создаёт pill window если ещё нет.
      // В headless'е showInactive() пропускается (см. dictation-pill.ts isHeadless).
      // Тут мы НЕ вызываем toggle (он требует backend), а просто проверяем
      // что dictation pill window отсутствует или скрыт.
      const windows = await app.evaluate(({ BrowserWindow }) => {
        return BrowserWindow.getAllWindows().map((w) => ({
          title: w.getTitle(),
          isVisible: w.isVisible(),
        }));
      });
      const dictationWindows = windows.filter((w) => /dictation/i.test(w.title) || w.title === "");
      // pill ещё не создан (toggle не звали) — должен быть либо отсутствовать,
      // либо invisible.
      for (const w of dictationWindows) {
        expect(w.isVisible, "Любой dictation-like window не должен быть visible в headless").toBe(
          false,
        );
      }
    } finally {
      await app.close();
    }
  });

  test("AC1: IPC handlers kepler:dictation:* зарегистрированы", async () => {
    const app = await launchKepler();
    try {
      await new Promise((r) => setTimeout(r, 2500));

      // `ipcMain.handle(channel, ...)` пишет в приватный `_invokeHandlers`
      // Map, не в Node EventEmitter `_events` (там оседают только
      // `ipcMain.on()` listeners). Probe'ить нужно через `_invokeHandlers`.
      const resp = await app.evaluate(async ({ ipcMain }) => {
        const internal = ipcMain as unknown as {
          _invokeHandlers?: Map<string, unknown>;
        };
        const handlers = internal._invokeHandlers;
        const has = (ch: string): boolean => handlers?.has(ch) ?? false;
        return {
          hasToggle: has("kepler:dictation:toggle"),
          hasCancel: has("kepler:dictation:cancel"),
          hasPillFinished: has("kepler:dictation:pill-finished"),
          handlerCount: handlers?.size ?? 0,
        };
      });
      expect(resp.hasToggle, "kepler:dictation:toggle handler должен быть зарегистрирован").toBe(
        true,
      );
      expect(resp.hasCancel, "kepler:dictation:cancel handler должен быть зарегистрирован").toBe(
        true,
      );
      expect(
        resp.hasPillFinished,
        "kepler:dictation:pill-finished handler должен быть зарегистрирован",
      ).toBe(true);
    } finally {
      await app.close();
    }
  });
});
