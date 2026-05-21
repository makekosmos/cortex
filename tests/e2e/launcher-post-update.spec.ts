// Post-update banner в LauncherView (Фича B).
//
// Два пути теста:
//   1) Прямой emit `kepler:post-update` event'а через main process →
//      banner появляется в DOM. Это покрывает renderer-side flow
//      (subscription + reactive render).
//   2) Integration: записываем `post-update.flag` в реальный userData
//      ДО старта main process'а — main.ts читает флаг, шлёт event,
//      удаляет файл. Это покрывает main-side flow.
//
// Backward-compat: legacy формат (bare timestamp как строка) тоже принят —
// см. main.ts:1226-1228 (контент игнорируется, важен факт существования файла).

import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  launchKepler,
  freshDataDir,
  launchKeplerWithDataDir,
} from "./helpers/launch";
import {
  emitPostUpdate,
  getUserDataDir,
} from "./helpers/update-state";
import { getLauncherWindow, waitForPostUpdateTile } from "./helpers/launcher";

test.describe("launcher post-update banner", () => {
  test("emit kepler:post-update → banner рендерится с версией", async () => {
    const app = await launchKepler({ slug: "post-update-emit" });
    try {
      const launcher = await getLauncherWindow(app);
      // Ждём пока renderer завершит onMounted (subscribe на postUpdate.onShown).
      await launcher.waitForTimeout(1000);

      await emitPostUpdate(app, { version: "9.9.9" });

      await waitForPostUpdateTile(launcher);
      const tile = launcher.locator(".post-update-tile");
      await expect(tile.locator(".update-title")).toHaveText(
        /Kepler обновл[её]н до v9\.9\.9/i,
      );
    } finally {
      await app.close();
    }
  });

  test("post-update.flag (JSON формат) → banner появляется после старта", async () => {
    // Готовим dataDir заранее, чтобы записать флаг ДО launch.
    const slug = "post-update-flag-json";
    const dataDir = freshDataDir(slug);
    const userdata = path.join(dataDir, "userdata");
    fs.mkdirSync(userdata, { recursive: true });
    const flag = path.join(userdata, "post-update.flag");
    fs.writeFileSync(flag, JSON.stringify({ at: Date.now() }), "utf8");

    // launchKepler сделал бы freshDataDir(slug) и снёс нашу подготовку —
    // используем low-level helper, который не чистит dir.
    const app = await launchKeplerWithDataDir(dataDir);

    try {
      const launcher = await getLauncherWindow(app);
      await waitForPostUpdateTile(launcher);

      const tile = launcher.locator(".post-update-tile");
      await expect(tile).toBeVisible();
      // Версия из app.getVersion() — в test bundle это валидная semver-строка
      // (см. shell/package.json). Не хардкодим — просто проверяем что не пусто.
      const title = await tile.locator(".update-title").innerText();
      expect(title).toMatch(/Kepler обновл[её]н до v\d+\.\d+\.\d+/i);

      // Флаг должен быть удалён.
      const realUserData = await getUserDataDir(app);
      const realFlag = path.join(realUserData, "post-update.flag");
      expect(fs.existsSync(realFlag)).toBe(false);
    } finally {
      await app.close();
    }
  });

  test("legacy bare-timestamp флаг тоже триггерит баннер", async () => {
    // Backwards-compat path: main.ts читает файл tolerant'но — формат не
    // парсится, важен только сам факт существования (main.ts:1223-1229).
    const slug = "post-update-flag-legacy";
    const dataDir = freshDataDir(slug);
    const userdata = path.join(dataDir, "userdata");
    fs.mkdirSync(userdata, { recursive: true });
    const flag = path.join(userdata, "post-update.flag");
    // Bare timestamp как строка — legacy формат.
    fs.writeFileSync(flag, String(Date.now()), "utf8");

    const app = await launchKeplerWithDataDir(dataDir);

    try {
      const launcher = await getLauncherWindow(app);
      await waitForPostUpdateTile(launcher);
      // Версия должна присутствовать (берётся из app.getVersion(), не из файла).
      const title = await launcher
        .locator(".post-update-tile .update-title")
        .innerText();
      expect(title).toMatch(/v\d+\.\d+\.\d+/);
    } finally {
      await app.close();
    }
  });

  test("без флага — post-update tile не показывается", async () => {
    const app = await launchKepler({ slug: "post-update-no-flag" });
    try {
      const launcher = await getLauncherWindow(app);
      // Дать main process'у возможность отработать post-update flow (нет файла →
      // ничего не шлёт).
      await launcher.waitForTimeout(1500);
      await expect(launcher.locator(".post-update-tile")).toHaveCount(0);
    } finally {
      await app.close();
    }
  });

  test("клик по post-update tile — dismiss'ит баннер", async () => {
    const app = await launchKepler({ slug: "post-update-dismiss" });
    try {
      const launcher = await getLauncherWindow(app);
      await launcher.waitForTimeout(1000);

      await emitPostUpdate(app, { version: "1.2.3" });
      await waitForPostUpdateTile(launcher);

      await launcher.locator(".post-update-tile").click();
      await expect(launcher.locator(".post-update-tile")).toHaveCount(0);
    } finally {
      await app.close();
    }
  });
});
