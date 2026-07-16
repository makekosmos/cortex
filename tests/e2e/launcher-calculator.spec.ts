import path from "node:path";
import fs from "node:fs";
import { expect, test } from "@playwright/test";
import { freshDataDir, launchKeplerWithDataDir, REPO_ROOT } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";

test("launcher evaluates and copies an inline calculation", async () => {
  const dataDir = freshDataDir("launcher-calculator");
  fs.writeFileSync(
    path.join(dataDir, "calculator-exchange-rates.json"),
    JSON.stringify({
      fetched_at: Math.floor(Date.now() / 1000),
      rates: { RUB: 1, USD: 1 / 90 },
    }),
  );
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    const launcher = await app.firstWindow();
    await launcher.waitForLoadState("domcontentloaded");
    await waitForBackendReady(launcher);
    await app.evaluate(({ clipboard }) => clipboard.writeText("before-calculation"));

    const search = launcher.locator(".search");
    await search.fill("10 km to miles");
    await expect(launcher.locator(".result--calculator")).toContainText("6.2137");

    await search.fill("22+4");
    const stableResult = launcher.locator(".result--calculator");
    await expect(stableResult.locator(".calculator-card__result")).toHaveText("26");
    await launcher.evaluate(() => {
      const state = { removed: false };
      (window as typeof window & { calculatorCardState?: typeof state }).calculatorCardState =
        state;
      new MutationObserver(() => {
        if (!document.querySelector(".result--calculator")) state.removed = true;
      }).observe(document.querySelector(".list")!, { childList: true, subtree: true });
    });
    await search.press("4");
    await expect(stableResult.locator(".calculator-card__result")).toHaveText("66");
    expect(
      await launcher.evaluate(
        () =>
          (window as typeof window & { calculatorCardState?: { removed: boolean } })
            .calculatorCardState?.removed,
      ),
    ).toBe(false);

    await search.fill("100 USD to RUB");
    await expect(stableResult.locator(".calculator-card__expression")).toHaveText("100 USD");
    await expect(stableResult.locator(".calculator-card__result")).toContainText("9000");

    await search.fill("900 долларов в рублях");
    await expect(stableResult.locator(".calculator-card__expression")).toHaveText("900 USD");
    await expect(stableResult.locator(".calculator-card__result")).toHaveText("81000.00 RUB");

    const result = launcher.locator(".result--calculator");
    await expect(
      launcher.locator(".section-label").filter({ hasText: "Калькулятор" }),
    ).toBeVisible();
    await expect(result).toContainText("Выражение");
    await expect(result).toContainText("Результат");
    await expect(result).toHaveClass(/selected/);

    const screenshotBase64 = await app.evaluate(async ({ BrowserWindow }) => {
      const window = BrowserWindow.getAllWindows()[0];
      if (!window) throw new Error("launcher window not found");
      const bounds = window.getBounds();
      window.setBounds({ ...bounds, width: bounds.width + 1 });
      window.setBounds(bounds);
      window.webContents.invalidate();
      await new Promise((resolve) => setTimeout(resolve, 100));
      return (await window.webContents.capturePage()).toPNG().toString("base64");
    });
    const screenshotPath = path.join(
      REPO_ROOT,
      ".tmp",
      "visual",
      "2026-07-15-launcher-calculator",
      "launcher-currency.png",
    );
    fs.mkdirSync(path.dirname(screenshotPath), { recursive: true });
    fs.writeFileSync(screenshotPath, Buffer.from(screenshotBase64, "base64"));

    await search.fill("1200 * 1.2");
    await expect(result.locator(".calculator-card__expression")).toHaveText("1200 * 1.2");
    await expect(result.locator(".calculator-card__result")).toHaveText("1440");
    await search.press("Enter");
    await expect.poll(() => app.evaluate(({ clipboard }) => clipboard.readText())).toBe("1440");
  } finally {
    await app.close();
  }
});
