import { test, expect } from "@playwright/test";
import { freshDataDir, launchKeplerWithDataDir } from "../../../tests/e2e/helpers/launch";

test.describe("settings about", () => {
  test("shows storage summary in About page", { timeout: 60_000 }, async () => {
    const dataDir = freshDataDir("settings-about-storage-summary");
    const app = await launchKeplerWithDataDir(dataDir);
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await launcher.waitForFunction(() => Boolean(window.kepler?.settings?.open), null, {
        timeout: 10_000,
      });

      await launcher.evaluate(async () => {
        await window.kepler.settings.open();
      });

      const settings = await app.waitForEvent("window", { timeout: 10_000 });
      await settings.waitForLoadState("domcontentloaded");
      await settings.getByRole("button", { name: "О приложении" }).click();

      await expect(settings.getByText("Данные", { exact: true })).toBeVisible();
      await expect(settings.getByText("Всего", { exact: true })).toBeVisible();
      await expect(settings.getByText("База ARK", { exact: true })).toBeVisible();
      await expect(settings.getByText("Локальные модели диктации", { exact: true })).toBeVisible();
      await expect(settings.getByText("Состояние окна и Chromium", { exact: true })).toBeVisible();
    } finally {
      await app.close();
    }
  });
});
