import path from "node:path";
import fs from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication, type Page } from "playwright";

const require = createRequire(import.meta.url);
const electronBinary = require("electron") as string;
const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const e2eRoot = path.join(appRoot, ".e2e");
const dbPath = path.join(e2eRoot, "horologion-e2e.db");

async function launchHorologion(): Promise<{ app: ElectronApplication; page: Page }> {
  fs.mkdirSync(e2eRoot, { recursive: true });
  // Чистая БД на каждый прогон.
  if (fs.existsSync(dbPath)) fs.rmSync(dbPath);

  const app = await electron.launch({
    executablePath: electronBinary,
    cwd: appRoot,
    args: [path.join(appRoot, "dist-electron", "main.js")],
    env: {
      ...process.env,
      ARK_DB_PATH: dbPath,
      NODE_ENV: "test",
    },
  });
  const page = await app.firstWindow();
  await page.waitForLoadState("domcontentloaded");
  return { app, page };
}

test.describe("horologion smoke", () => {
  test("renders top bar, creates time entry on play, stops on pause", async () => {
    const { app, page } = await launchHorologion();
    try {
      // Input + кнопка play видимы под titlebar'ом
      const input = page.locator(".inputbar__input");
      await expect(input).toBeVisible();

      const playBtn = page.locator(".playbtn");
      await expect(playBtn).toBeVisible();

      // empty state
      await expect(page.locator(".empty")).toContainText("Записей пока нет");

      // Стартуем таймер
      await input.fill("E2E test entry");
      await playBtn.click();

      // После старта кнопка превращается в pause — это сигнал что запись идёт.
      await expect(page.locator(".playbtn--running")).toBeVisible({ timeout: 5_000 });

      // Останавливаем — кнопка стала pause
      await playBtn.click();

      // После стопа запись должна появиться в списке
      await expect(page.locator(".row__title").first()).toContainText("E2E test entry", {
        timeout: 5_000,
      });
    } finally {
      await app.close();
    }
  });
});
