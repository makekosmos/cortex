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
  test("HomeView рендерится: draft input + mode toggle + empty list", async () => {
    const { app, page } = await launchHorologion();
    try {
      // Поле «Над чем работаем?» — draft input card сверху.
      await expect(page.locator(".pdi__input")).toBeVisible();

      // Шайба-переключатель с двумя кнопками.
      const toggleButtons = page.locator(".mode-toggle__btn");
      await expect(toggleButtons).toHaveCount(2);
      await expect(toggleButtons.nth(0)).toContainText("Помодоро");
      await expect(toggleButtons.nth(1)).toContainText("Секундомер");

      // По умолчанию активен Помодоро — pomo-ring виден.
      await expect(page.locator(".pomo__ring")).toBeVisible();

      // Список пуст.
      await expect(page.locator(".empty")).toContainText("Записей пока нет");
    } finally {
      await app.close();
    }
  });

  test("переключение Помодоро ↔ Секундомер меняет видимый таймер", async () => {
    const { app, page } = await launchHorologion();
    try {
      // Стартуем на Помодоро.
      await expect(page.locator(".pomo__ring")).toBeVisible();

      // Переключаемся на Секундомер.
      await page.locator(".mode-toggle__btn", { hasText: "Секундомер" }).click();
      await expect(page.locator(".sw__time")).toBeVisible({ timeout: 1_000 });
      await expect(page.locator(".pomo__ring")).toHaveCount(0);

      // Назад на Помодоро.
      await page.locator(".mode-toggle__btn", { hasText: "Помодоро" }).click();
      await expect(page.locator(".pomo__ring")).toBeVisible({ timeout: 1_000 });
    } finally {
      await app.close();
    }
  });

  test("Секундомер создаёт time_entry при старте, останавливает при стопе", async () => {
    const { app, page } = await launchHorologion();
    try {
      // Переключаемся на Секундомер.
      await page.locator(".mode-toggle__btn", { hasText: "Секундомер" }).click();
      const time = page.locator(".sw__time");
      await expect(time).toBeVisible();
      await expect(time).toContainText("00:00:00");

      // Пишем title и стартуем сессию.
      await page.locator(".pdi__input").fill("E2E test entry");
      await page.locator(".sw__primary").click();

      // Кнопка переключилась в --running (Стоп).
      await expect(page.locator(".sw__primary--running")).toBeVisible({ timeout: 5_000 });
      // Тайм-лейбл начал тикать (на 1+ секунду).
      await expect(time).not.toContainText("00:00:00", { timeout: 3_000 });

      // Стопаем.
      await page.locator(".sw__primary--running").click();

      // Запись появилась в списке.
      await expect(page.locator(".row__title").first()).toContainText("E2E test entry", {
        timeout: 5_000,
      });
    } finally {
      await app.close();
    }
  });

  test("Клик по заголовку дня сворачивает/раскрывает список записей", async () => {
    const { app, page } = await launchHorologion();
    try {
      // Создаём одну запись через Секундомер чтобы появился day-header.
      await page.locator(".mode-toggle__btn", { hasText: "Секундомер" }).click();
      await page.locator(".pdi__input").fill("Test entry");
      await page.locator(".sw__primary").click();
      await page.waitForTimeout(1_500);
      await page.locator(".sw__primary--running").click();
      await expect(page.locator(".row__title").first()).toBeVisible({ timeout: 5_000 });

      // Day header кликабельный — сворачиваем.
      const dayHead = page.locator(".day__head").first();
      await dayHead.click();
      await expect(page.locator(".day--collapsed")).toBeVisible({ timeout: 1_000 });

      // Раскрываем обратно.
      await dayHead.click();
      await expect(page.locator(".day--collapsed")).toHaveCount(0);
    } finally {
      await app.close();
    }
  });
});
