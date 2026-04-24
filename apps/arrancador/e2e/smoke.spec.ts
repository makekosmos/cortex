import { expect, test } from "@playwright/test";
import {
  type AppSettings,
  testFavoriteGameFixture,
  testGameFixture,
} from "../src/types";
import { bridgeMockInit } from "./bridge-mock";

const mockSettings: AppSettings = {
  theme: "dark",
  ludusavi_path: "native",
  backup_directory: "C:\\Backups",
  auto_backup: true,
  backup_before_launch: true,
  backup_compression_enabled: true,
  backup_compression_level: 60,
  backup_skip_compression_once: false,
  max_backups_per_game: 5,
  rawg_api_key: "",
  start_minimized_in_tray: false,
};

const mockConfig = {
  games: [testGameFixture, testFavoriteGameFixture],
  settings: mockSettings,
  scanEntries: [
    {
      path: "C:\\Games\\Elysium\\Elysium.exe",
      file_name: "Elysium.exe",
    },
  ],
  processes: [
    {
      pid: 4242,
      name: "Elysium.exe",
      path: "C:\\Games\\Elysium\\Elysium.exe",
      cpu_usage: 12.3,
      gpu_usage: 0,
    },
  ],
  dialogOpenResult: "C:\\Games",
};

test.beforeEach(async ({ page }) => {
  await page.addInitScript(bridgeMockInit, mockConfig);
});

test("library loads and settings navigation works", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Arcadia")).toBeVisible();

  await page.locator('a[href="#/settings"]').click();
  await expect(page.locator("#setting-autostart")).toBeVisible();

  await page.locator('a[href="#/"]').first().click();
  await expect(page.getByText("Arcadia")).toBeVisible();
});

test("scan flow emits entries from mocked backend", async ({ page }) => {
  await page.goto("/#/scan");
  await page.getByTestId("scan-start").click();

  await expect(page.getByTestId("scan-entry-name").first()).toHaveValue(
    "Elysium",
  );
});
