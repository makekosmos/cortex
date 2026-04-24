import { type Browser, chromium, expect, type Page } from "@playwright/test";
import { bridgeMockInit } from "../e2e/bridge-mock";
import {
  type AppSettings,
  type RawgGame,
  testFavoriteGameFixture,
  testGameFixture,
} from "../src/types";

const baseURL = process.env.ARRANCADOR_E2E_BASE_URL ?? "http://127.0.0.1:4174";

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

const rawgItems: RawgGame[] = [
  {
    id: 2201,
    name: "Sky Harbor",
    slug: "sky-harbor",
    released: "2024-03-12",
    background_image: null,
    metacritic: 84,
    rating: 4.2,
    ratings_count: 1200,
    genres: [{ id: 1, name: "Adventure", slug: "adventure" }],
    platforms: [{ platform: { id: 4, name: "PC", slug: "pc" } }],
  },
  {
    id: 2202,
    name: "Luna Forge",
    slug: "luna-forge",
    released: "2025-05-06",
    background_image: null,
    metacritic: 90,
    rating: 4.7,
    ratings_count: 850,
    genres: [{ id: 2, name: "RPG", slug: "rpg" }],
    platforms: [{ platform: { id: 4, name: "PC", slug: "pc" } }],
  },
];

const baseBridgeConfig = {
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

async function withMockedPage<T>(
  browser: Browser,
  config: Record<string, unknown>,
  runScenario: (page: Page) => Promise<T>,
) {
  const context = await browser.newContext();
  const page = await context.newPage();

  await page.addInitScript(bridgeMockInit, config);

  try {
    return await runScenario(page);
  } finally {
    await context.close();
  }
}

async function runSmokeNavigation(browser: Browser) {
  await withMockedPage(browser, baseBridgeConfig, async (page) => {
    await page.goto(`${baseURL}/#/`);
    await expect(page.getByText("Arcadia")).toBeVisible();

    await page.locator('a[href="#/settings"]').click();
    await expect(page.locator("#setting-autostart")).toBeVisible();

    await page.locator('a[href="#/"]').first().click();
    await expect(page.getByText("Arcadia")).toBeVisible();
  });
}

async function runScanFlow(browser: Browser) {
  await withMockedPage(browser, baseBridgeConfig, async (page) => {
    await page.goto(`${baseURL}/#/scan`);
    await page.getByTestId("scan-start").click();
    await expect(page.getByTestId("scan-entry-name").first()).toHaveValue("Elysium");
  });
}

async function runCatalogueFlow(browser: Browser) {
  await withMockedPage(
    browser,
    {
      ...baseBridgeConfig,
      rawgItems,
    },
    async (page) => {
      await page.goto(`${baseURL}/#/catalogue`);

      await expect(
        page.getByRole("heading", { name: "Game Catalogue" }),
      ).toBeVisible();
      await expect(page.getByTestId("catalogue-card-2201")).toContainText("Sky Harbor");

      await page.getByTestId("catalogue-search-input").fill("luna");
      await page.getByTestId("catalogue-search-button").click();

      await expect(page.getByTestId("catalogue-card-2202")).toContainText("Luna Forge");
      await expect(page.getByTestId("catalogue-card-2201")).not.toBeVisible();

      await page.getByTestId("catalogue-add-2202").click();
      await page.evaluate(() => {
        window.location.hash = "#/";
      });
      await expect(page.getByText("Luna Forge").first()).toBeVisible();
    },
  );
}

async function runNoWebSocketCheck(browser: Browser) {
  await withMockedPage(
    browser,
    {
      ...baseBridgeConfig,
      rawgItems,
    },
    async (page) => {
      let websocketCount = 0;
      page.on("websocket", () => {
        websocketCount += 1;
      });

      await page.goto(`${baseURL}/#/catalogue`);
      expect(websocketCount).toBe(0);
    },
  );
}

const browser = await chromium.launch({
  headless: true,
});

try {
  await runSmokeNavigation(browser);
  await runScanFlow(browser);
  await runCatalogueFlow(browser);
  await runNoWebSocketCheck(browser);
  console.log("[e2e-inline] 4 scenarios passed");
} finally {
  await browser.close();
}
