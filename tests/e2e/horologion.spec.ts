// Horologion extension smoke (AC3 / AC4 ranges).
//
// Skeleton: открывает Horologion через command bus, проверяет наличие
// двух базовых режимов в UI — «Помодоро» и «Секундомер». Полные AC3/AC4
// (timer behaviour, pomodoro auto-break) — отдельные тесты позже.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

const REQUIRED_TOGGLES = ["Помодоро", "Секундомер"];

test.describe("horologion extension", () => {
  test("horologion: UI содержит переключатели режимов", async () => {
    const app = await launchKepler({ slug: "horologion-modes" });
    try {
      await new Promise((r) => setTimeout(r, 2000));

      const opened = await app.evaluate(async ({ BrowserWindow }, commandId) => {
        const wins = BrowserWindow.getAllWindows();
        const launcher = wins[0];
        if (!launcher) return { triggered: false };
        await launcher.webContents.executeJavaScript(
          `window.kepler?.commands?.invoke?.(${JSON.stringify(commandId)})`,
        );
        return { triggered: true };
      }, "horologion:open");
      expect(opened.triggered).toBe(true);

      const horoWindow = await app.waitForEvent("window", { timeout: 10_000 });
      await horoWindow.waitForLoadState("domcontentloaded");
      await horoWindow.waitForTimeout(1500);

      const bodyText = (await horoWindow.locator("body").textContent()) ?? "";
      const missing = REQUIRED_TOGGLES.filter((t) => !bodyText.includes(t));
      if (missing.length > 0) {
        throw new Error(
          `Horologion UI не содержит обязательные toggle: [${missing.join(", ")}]. ` +
            `Body text (200 chars): ${bodyText.slice(0, 200)}`,
        );
      }
      for (const t of REQUIRED_TOGGLES) {
        expect(bodyText).toContain(t);
      }
    } finally {
      await app.close();
    }
  });
});
