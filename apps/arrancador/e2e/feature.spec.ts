import { expect, test } from "@playwright/test";
import type { AppSettings, RawgGame } from "../src/types";
import { testFavoriteGameFixture, testGameFixture } from "../src/types";
import { bridgeMockInit } from "./bridge-mock";

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

const baseConfig = {
  games: [testGameFixture, testFavoriteGameFixture],
  settings: {
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
  } as AppSettings,
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
  rawgItems,
};

test.beforeEach(async ({ page }) => {
  await page.addInitScript(bridgeMockInit, {
    ...baseConfig,
  });
});

test.describe("catalogue page", () => {
  test("loads RAWG catalogue, searches, and adds a game to the library", async ({
    page,
  }) => {
    await page.goto("/#/catalogue");

    await expect(
      page.getByRole("heading", { name: "Game Catalogue" }),
    ).toBeVisible();
    await expect(page.getByTestId("catalogue-card-2201")).toContainText(
      "Sky Harbor",
    );

    await page.getByTestId("catalogue-search-input").fill("luna");
    await page.getByTestId("catalogue-search-button").click();

    await expect(page.getByTestId("catalogue-card-2202")).toContainText(
      "Luna Forge",
    );
    await expect(page.getByTestId("catalogue-card-2201")).not.toBeVisible();

    await page.getByTestId("catalogue-add-2202").click();
    await page.evaluate(() => {
      window.location.hash = "#/";
    });
    await expect(page.getByText("Luna Forge").first()).toBeVisible();
  });
});

test("no websocket connections are opened while using local pages", async ({ page }) => {
  let websocketCount = 0;
  page.on("websocket", () => {
    websocketCount += 1;
  });

  await page.goto("/#/catalogue");

  expect(websocketCount).toBe(0);
});
