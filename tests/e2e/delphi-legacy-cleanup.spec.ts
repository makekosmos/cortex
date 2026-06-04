import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { launchKepler } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";

const VISUAL_DIR = path.resolve(".tmp", "visual", "2026-06-04-delphi-legacy-cleanup");

async function openDelphi(app: Awaited<ReturnType<typeof launchKepler>>) {
  await app.evaluate(async ({ ipcMain }) => {
    const handlers = (
      ipcMain as unknown as {
        _invokeHandlers: Map<string, (...args: unknown[]) => unknown>;
      }
    )._invokeHandlers;
    const handler = handlers?.get?.("kepler:extension:open");
    if (!handler) throw new Error("kepler:extension:open handler not registered");
    await handler({} as never, "delphi");
  });
  const page = await app.waitForEvent("window", { timeout: 10_000 });
  await page.waitForLoadState("domcontentloaded");
  await page.getByRole("heading", { name: "Входящие" }).waitFor({ timeout: 10_000 });
  return page;
}

async function captureExtensionWindow(
  app: Awaited<ReturnType<typeof launchKepler>>,
  urlPart: string,
  screenshotPath: string,
) {
  const base64 = await app.evaluate(async ({ BrowserWindow }, part) => {
    const win = BrowserWindow.getAllWindows().find((candidate) =>
      candidate.webContents.getURL().includes(part),
    );
    if (!win) throw new Error(`window not found for ${part}`);
    const image = await win.capturePage();
    return image.toPNG().toString("base64");
  }, urlPart);
  fs.writeFileSync(screenshotPath, Buffer.from(base64, "base64"));
}

test.describe("delphi legacy cleanup", () => {
  test("opens directly into task UI without legacy spaces setup", async () => {
    test.setTimeout(60_000);
    fs.mkdirSync(VISUAL_DIR, { recursive: true });
    const app = await launchKepler({ slug: "delphi-legacy-cleanup" });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      const delphi = await openDelphi(app);
      const bodyText = (await delphi.locator("body").textContent()) ?? "";

      expect(bodyText).toContain("Входящие");
      expect(bodyText).toContain("Сегодня");
      expect(bodyText).not.toContain("Создать пространство");
      expect(bodyText).not.toContain("Сохранённые пространства");
      expect(bodyText).not.toContain("Пространства");

      await captureExtensionWindow(
        app,
        "extensions/delphi",
        path.join(VISUAL_DIR, "delphi-main-no-space-setup.png"),
      );
    } finally {
      await app.close();
    }
  });
});
