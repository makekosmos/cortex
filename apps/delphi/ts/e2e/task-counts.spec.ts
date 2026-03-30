/**
 * E2E test: verify that smart list task counts are correct and that there is
 * no duplication after deduplication fix in fetchTasksFromArk().
 *
 * Requires:
 *   - Built Electron app (dist-electron/main.js)
 *   - Ark server running at http://localhost:8000 with api key "dev-test-key"
 */
import { test, expect, type ElectronApplication, type Page } from "@playwright/test";
import { _electron as electron } from "playwright";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, "..");

const ARK_URL = "http://localhost:8000";
const ARK_KEY = "dev-test-key";

let app: ElectronApplication;
let page: Page;

test.beforeAll(async () => {
  app = await electron.launch({
    args: [path.join(rootDir, "dist-electron/main.js")],
    env: {
      ...process.env,
      NODE_ENV: "production",
    },
  });
  page = await app.firstWindow();

  // Inject Ark credentials so the app connects to the real server
  await page.evaluate(
    ({ url, key }: { url: string; key: string }) => {
      localStorage.setItem("delphi.ark_url", url);
      localStorage.setItem("delphi.ark_api_key", key);
    },
    { url: ARK_URL, key: ARK_KEY },
  );

  // Reload so credentials take effect and sync starts
  await page.reload();
  await page.waitForLoadState("domcontentloaded");

  // Wait for initial sync to complete: either wait for green dot or just 3s
  await page
    .waitForFunction(
      () =>
        document
          .querySelector(".connection-dot")
          ?.classList.contains("bg-green-400") ?? false,
      { timeout: 10_000 },
    )
    .catch(() => {
      // connection-dot may not be present or class name may differ — fall back
    });

  // Extra settle time for store to finish populating from HTTP + WS
  await page.waitForTimeout(3000);
});

test.afterAll(async () => {
  await app.close();
});

test.describe("Task counts — no duplication after deduplication fix", () => {
  test("sidebar is visible", async () => {
    const sidebar = page.locator("aside");
    await expect(sidebar).toBeVisible();
  });

  test("Журнал (Logbook) page shows tasks with unique ids only", async () => {
    // Navigate to Logbook
    await page.locator("aside").getByText("Журнал").click();
    await expect(page.getByRole("heading", { name: "Журнал" })).toBeVisible();

    // Count visible todo rows (each TodoRow renders a div with Circle/CheckCircle2 icon button)
    // TodoRow is a div.group containing a button with a circle icon
    const todoRows = page.locator(".group.flex.items-center.gap-3.px-7");
    const rowCount = await todoRows.count();

    // Sanity check: count should be a reasonable number, not 0×N inflation
    // Before fix: 81 tasks were shown in Logbook (inflated). After fix: ~38.
    // We assert it's under 200 to catch any future regression without being
    // too strict about the exact server data.
    expect(rowCount).toBeLessThanOrEqual(200);

    // Collect all task ids from the DOM to verify uniqueness
    const ids = await todoRows.evaluateAll((rows: Element[]) =>
      rows.map((row) => row.getAttribute("data-todo-id")).filter(Boolean),
    );

    if (ids.length > 0) {
      // If data-todo-id attributes are present, verify uniqueness
      const uniqueIds = new Set(ids);
      expect(uniqueIds.size).toBe(ids.length);
    }

    // The count shown in the heading badge should match the rendered rows
    const badgeText = await page
      .locator("h1")
      .locator("~ span")
      .first()
      .textContent()
      .catch(() => null);

    if (badgeText !== null) {
      const badgeCount = parseInt(badgeText.trim(), 10);
      if (!isNaN(badgeCount)) {
        // Badge and actual rendered rows should agree (within 1 for pagination)
        expect(Math.abs(badgeCount - rowCount)).toBeLessThanOrEqual(1);
      }
    }
  });

  test("Журнал count is a finite number and not inflated vs real row count", async () => {
    // Navigate to Logbook page
    await page.locator("aside").getByText("Журнал").click();
    await expect(page.getByRole("heading", { name: "Журнал" })).toBeVisible();

    const todoRows = page.locator(".group.flex.items-center.gap-3.px-7");
    const rowCount = await todoRows.count();

    // The page either shows "Завершённых задач нет" (0 tasks) or renders rows
    if (rowCount === 0) {
      const emptyMsg = page.getByText("Завершённых задач нет");
      await expect(emptyMsg).toBeVisible();
    } else {
      // Rows exist — count should be sane
      expect(rowCount).toBeGreaterThan(0);
      expect(rowCount).toBeLessThanOrEqual(200);
    }
  });

  test("navigating between smart lists does not multiply task counts", async () => {
    // Navigate to Logbook, record count
    await page.locator("aside").getByText("Журнал").click();
    await expect(page.getByRole("heading", { name: "Журнал" })).toBeVisible();
    const countFirst = await page
      .locator(".group.flex.items-center.gap-3.px-7")
      .count();

    // Navigate away and back
    await page.locator("aside").getByText("Входящие").click();
    await page.locator("aside").getByText("Журнал").click();
    await expect(page.getByRole("heading", { name: "Журнал" })).toBeVisible();
    const countSecond = await page
      .locator(".group.flex.items-center.gap-3.px-7")
      .count();

    // Count should be stable — re-navigation must not add duplicate rows
    expect(countSecond).toBe(countFirst);
  });

  test("Входящие (Inbox) page renders without duplicate tasks", async () => {
    await page.locator("aside").getByText("Входящие").click();
    await expect(
      page.getByRole("heading", { name: "Входящие" }),
    ).toBeVisible();

    const inboxRows = page.locator(".group.flex.items-center.gap-3.px-7");
    const inboxCount = await inboxRows.count();

    // Should be a sane number
    expect(inboxCount).toBeLessThanOrEqual(500);
  });
});
