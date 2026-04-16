import path from "node:path";
import fs from "node:fs";
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { _electron as electron } from "playwright";

const require = createRequire(import.meta.url);
const electronBinary = require("electron") as string;
const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dbPath = path.join(appRoot, ".e2e", "smoke-dashboard.db");

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

fs.mkdirSync(path.dirname(dbPath), { recursive: true });

execFileSync(
  "python",
  [path.join(appRoot, "scripts", "seedSmokeDb.py"), "--db-path", dbPath],
  {
    cwd: appRoot,
    stdio: "inherit",
    env: process.env,
  },
);

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

try {
  const page = await app.firstWindow();
  await page.waitForLoadState("domcontentloaded");
  await page.getByTestId("overview-page").waitFor();
  await page.getByTestId("top-apps-list").waitFor();

  const statusText = (await page.getByTestId("dashboard-status").getAttribute("aria-label")) ?? "";
  const dbPathText = (await page.getByTestId("dashboard-db-path").textContent()) ?? "";
  const topAppText = (await page.getByTestId("top-app-smoke-odyssey").textContent()) ?? "";

  assert(statusText.length > 0, "Overview did not expose dashboard status text");
  assert(dbPathText.includes("smoke-dashboard.db"), "Dashboard did not show the seeded DB path");
  assert(topAppText.includes("Odyssey Browser"), "Top apps list did not render the seeded app");

  await app.evaluate(async ({ BrowserWindow }) => {
    const win = BrowserWindow.getAllWindows()[0];
    win.setSize(1180, 720);
    await new Promise((resolve) => setTimeout(resolve, 100));
  });

  const scrollPane = page.getByTestId("dashboard-scroll-pane");
  await scrollPane.waitFor();

  const scrollMetrics = await scrollPane.evaluate((node) => ({
    scrollHeight: node.scrollHeight,
    clientHeight: node.clientHeight,
  }));
  assert(
    scrollMetrics.scrollHeight > scrollMetrics.clientHeight,
    "Dashboard scroll pane does not overflow vertically",
  );

  await scrollPane.evaluate((node) => {
    node.scrollTo({ top: 320, behavior: "instant" });
  });

  const scrolledTop = await scrollPane.evaluate((node) => node.scrollTop);
  assert(scrolledTop > 0, "Dashboard scroll pane did not move after scrollTo");

  await page.evaluate(() => {
    localStorage.removeItem("dashboard-sidebar-config");
    window.location.reload();
  });
  await page.waitForLoadState("domcontentloaded");
  await page.getByTestId("overview-page").waitFor();

  const sidebar = page.getByTestId("kepler-sidebar");
  const handle = page.getByTestId("kepler-sidebar-resize-handle");
  await sidebar.waitFor();
  await handle.waitFor();

  const beforeBox = await sidebar.boundingBox();
  assert(beforeBox, "Sidebar bounding box is unavailable");

  const handleBox = await handle.boundingBox();
  assert(handleBox, "Sidebar resize handle bounding box is unavailable");

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

  const resizedWidth = await page.waitForFunction(() => {
    const element = document.querySelector<HTMLElement>("[data-testid='kepler-sidebar']");
    return element?.getBoundingClientRect().width ?? 0;
  });
  const resizedWidthValue = await resizedWidth.jsonValue() as number;
  assert(
    resizedWidthValue > beforeBox.width + 20,
    `Sidebar width did not increase after drag: before=${beforeBox.width}, after=${resizedWidthValue}`,
  );

  await page.evaluate(() => {
    window.location.hash = "#/sessions";
  });
  await page.getByTestId("sessions-page").waitFor();
  await page.getByTestId("recent-session-smoke-odyssey-session-0").waitFor();

  console.log(
    JSON.stringify(
      {
        status: "ok",
        dbPath,
        statusText,
        topApp: "Odyssey Browser",
        scrolledTop,
        resizedWidth: resizedWidthValue,
        route: "sessions",
      },
      null,
      2,
    ),
  );
} finally {
  await app.close();
}
