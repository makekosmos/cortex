import { expect, type Page, test } from "@playwright/test";
import type { AppSettings, testFavoriteGameFixture, testGameFixture } from "../src/types";
import { bridgeMockInit } from "./tauri-mock";

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
};

const attachClipboardRecorder = async (page: Page) => {
  await page.addInitScript(() => {
    (window as any).__clipboardWrites = [];
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async (value: string) => {
          (window as any).__clipboardWrites.push(value);
          return Promise.resolve();
        },
      },
    });
  });
};

const getLastClipboardWrite = async (page: Page) =>
  page.evaluate(() => {
    const writes = (window as any).__clipboardWrites as string[] | undefined;
    return writes?.at(-1) ?? "";
  });

test.beforeEach(async ({ page }) => {
  await page.addInitScript(bridgeMockInit, {
    ...baseConfig,
  });
});

test.describe("catalogue page", () => {
  test("covers full lifecycle and sync/filter", async ({ page }) => {
    await attachClipboardRecorder(page);

    page.on("dialog", (dialog) => {
      if (dialog.type() === "confirm") {
        void dialog.accept();
      }
    });

    await page.goto("/catalogue");
    await expect(
      page.getByText("No catalogue items yet. Add one using the form above."),
    ).toBeVisible();

    await page.getByTestId("catalogue-rawg-id-input").fill("1201");
    await page.getByTestId("catalogue-name-input").fill("Local item");
    await page.getByTestId("catalogue-source-input").fill("manual");
    await page.getByTestId("catalogue-payload-input").fill("{ invalid");
    await page.getByTestId("catalogue-save-button").click();
    await expect(page.getByText("Payload must be valid JSON")).toBeVisible();

    await page.getByTestId("catalogue-payload-input").fill('{"tier":"manual"}');
    await page.getByTestId("catalogue-save-button").click();
    await expect(page.getByText("Local item")).toBeVisible();

    const editButton = page.getByTestId(/catalogue-edit-/).first();
    await editButton.click();
    await expect(page.getByTestId("catalogue-name-input")).toHaveValue("Local item");
    await page.getByTestId("catalogue-name-input").fill("Local item v2");
    await page.getByTestId("catalogue-save-button").click();
    await expect(page.getByText("Local item v2")).toBeVisible();
    await expect(page.getByText("Local item")).not.toBeVisible();

    const copyButton = page.getByTestId(/catalogue-copy-/).first();
    await copyButton.click();
    expect(await getLastClipboardWrite(page)).toContain('"tier":"manual"');

    await page.getByTestId("catalogue-search-input").fill("Local");
    await page.getByTestId("catalogue-search-button").click();
    await expect(page.getByText("Local item v2")).toBeVisible();

    await page.getByTestId("catalogue-sync-button").click();
    await page.getByTestId("catalogue-source-filter").selectOption("library");
    await page.getByTestId("catalogue-reload-button").click();
    await expect(page.getByText("Arcadia")).toBeVisible();
    await expect(page.getByText("Bastion")).toBeVisible();

    await page.getByTestId("catalogue-source-filter").selectOption("manual");
    await page.getByTestId("catalogue-reload-button").click();
    await expect(page.getByText("Local item v2")).toBeVisible();

    await page.getByTestId(/catalogue-delete-/).first().click();
    await expect(page.getByText("Local item v2")).not.toBeVisible();
    await expect(
      page.getByText("No catalogue items yet. Add one using the form above."),
    ).toBeVisible();
  });
});

test("no websocket connections are opened while using local pages", async ({ page }) => {
  let websocketCount = 0;
  page.on("websocket", () => {
    websocketCount += 1;
  });

  await page.goto("/catalogue");

  expect(websocketCount).toBe(0);
});

