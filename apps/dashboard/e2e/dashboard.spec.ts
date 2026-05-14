import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication, type Page } from "playwright";

const require = createRequire(import.meta.url);
const electronBinary = require("electron") as string;
const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dbPath = path.join(appRoot, ".e2e", "smoke-dashboard.db");

async function launchDashboard(): Promise<{ app: ElectronApplication; page: Page }> {
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

async function resizeWindow(app: ElectronApplication, width: number, height: number) {
  await app.evaluate(
    async ({ BrowserWindow }, { width, height }) => {
      const win = BrowserWindow.getAllWindows()[0];
      win.setSize(width, height);
      await new Promise((resolve) => setTimeout(resolve, 100));
    },
    { width, height },
  );
}

test("renders overview metrics from Ark DB", async () => {
  const { app, page } = await launchDashboard();

  try {
    await expect(page.getByTestId("overview-page")).toBeVisible();
    await expect(page.getByTestId("dashboard-status")).toHaveAttribute("aria-label", /.+/);
    await expect(page.getByTestId("dashboard-db-path")).toContainText("smoke-dashboard.db");
    await expect(page.getByTestId("top-apps-list")).toBeVisible();
    await expect(page.getByTestId("top-app-smoke-odyssey")).toContainText("Odyssey Browser");
    await expect(page.getByTestId("metric-focus-time")).toBeVisible();
  } finally {
    await app.close();
  }
});

test("renders sessions route from the same seeded Ark DB", async () => {
  const { app, page } = await launchDashboard();

  try {
    await page.evaluate(() => {
      window.location.hash = "#/sessions";
    });

    await expect(page.getByTestId("sessions-page")).toBeVisible();
    await expect(page.getByTestId("recent-sessions-table")).toBeVisible();
    await expect(page.getByTestId("recent-session-smoke-odyssey-session-0")).toBeVisible();
  } finally {
    await app.close();
  }
});

test("overview content scrolls inside the main dashboard pane", async () => {
  const { app, page } = await launchDashboard();

  try {
    await resizeWindow(app, 1180, 720);

    const scrollPane = page.getByTestId("dashboard-scroll-pane");
    await expect(scrollPane).toBeVisible();

    await page.waitForFunction(() => {
      const element = document.querySelector<HTMLElement>("[data-testid='dashboard-scroll-pane']");
      return Boolean(element && element.scrollHeight > element.clientHeight);
    });

    const before = await scrollPane.evaluate((node) => ({
      scrollTop: node.scrollTop,
      scrollHeight: node.scrollHeight,
      clientHeight: node.clientHeight,
    }));

    expect(before.scrollHeight).toBeGreaterThan(before.clientHeight);

    await scrollPane.evaluate((node) => {
      node.scrollTo({ top: 320, behavior: "instant" });
    });

    await expect
      .poll(async () => scrollPane.evaluate((node) => node.scrollTop))
      .toBeGreaterThan(0);
  } finally {
    await app.close();
  }
});

test("sidebar resize handle changes dashboard sidebar width", async () => {
  const { app, page } = await launchDashboard();

  try {
    await resizeWindow(app, 1440, 900);

    await page.evaluate(() => {
      localStorage.removeItem("dashboard-sidebar-config");
      window.location.reload();
    });
    await page.waitForLoadState("domcontentloaded");
    await expect(page.getByTestId("overview-page")).toBeVisible();

    const sidebar = page.getByTestId("kosmos-sidebar");
    const handle = page.getByTestId("kosmos-sidebar-resize-handle");

    await expect(sidebar).toBeVisible();
    await expect(handle).toBeVisible();

    const beforeBox = await sidebar.boundingBox();
    if (!beforeBox) {
      throw new Error("Sidebar bounding box is unavailable");
    }

    const handleBox = await handle.boundingBox();
    if (!handleBox) {
      throw new Error("Sidebar resize handle bounding box is unavailable");
    }

    await page.mouse.move(
      handleBox.x + handleBox.width / 2,
      handleBox.y + handleBox.height / 2,
    );
    await page.mouse.down();
    await page.mouse.move(
      handleBox.x + handleBox.width / 2 + 72,
      handleBox.y + handleBox.height / 2,
      { steps: 8 },
    );
    await page.mouse.up();

    await expect
      .poll(async () => {
        const box = await sidebar.boundingBox();
        return box?.width ?? 0;
      })
      .toBeGreaterThan(beforeBox.width + 20);
  } finally {
    await app.close();
  }
});
