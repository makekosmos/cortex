// Visual regression snapshots (Phase 6 bug-detection roadmap).
//
// Цель — дешёвая защита от CSS-регрессий. typecheck не ловит «layout
// поехал», unit-тесты не ловят «цвета перепутаны». Playwright
// `toHaveScreenshot` делает pixel-diff против baseline закоммиченного
// в репо.
//
// Платформа: Windows-only (snapshot создан на Windows, font hinting и
// rendering нестабилен cross-OS). `maxDiffPixels: 200` — небольшой
// запас на subpixel differences между Windows builds.
//
// Окна Kepler в e2e создаются `show: false` (headless mode) — screenshot
// всё равно работает через webContents.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";

const SNAP_OPTS = { maxDiffPixels: 200 } as const;

test.describe("visual regression", () => {
  test("launcher: initial state", async () => {
    const app = await launchKepler({ slug: "visual-launcher" });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);
      // Стабилизируем перед скрином: дать Vue завершить mount.
      await launcher.waitForLoadState("networkidle").catch(() => {});
      await launcher.evaluate(() => document.fonts?.ready ?? Promise.resolve());
      await expect(launcher).toHaveScreenshot("launcher-initial.png", SNAP_OPTS);
    } finally {
      await app.close();
    }
  });

  test("eden: empty journal view", async () => {
    const app = await launchKepler({ slug: "visual-eden" });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      const triggered = await launcher.evaluate(async () => {
        if (typeof window.kepler?.commands?.invoke !== "function") return "no-api";
        try {
          await window.kepler.commands.invoke("eden:open");
          return "ok";
        } catch (e) {
          return "throw:" + (e instanceof Error ? e.message : String(e));
        }
      });
      expect(triggered).toBe("ok");

      const edenWindow = await app.waitForEvent("window", { timeout: 10_000 });
      await edenWindow.waitForLoadState("domcontentloaded");
      await waitForBackendReady(edenWindow);
      await edenWindow.waitForFunction(
        () =>
          typeof (window as unknown as { api?: { saveEntry?: unknown } }).api?.saveEntry ===
          "function",
        null,
        { timeout: 5_000 },
      );
      // Дать Vue полностью смонтировать журнальный экран. Eden — heavy mount
      // (TipTap), нужен большой запас.
      await edenWindow.evaluate(() => document.fonts?.ready ?? Promise.resolve());
      await edenWindow.waitForTimeout(2000);
      await expect(edenWindow).toHaveScreenshot("eden-journal-empty.png", {
        ...SNAP_OPTS,
        timeout: 15_000,
      });
    } finally {
      await app.close();
    }
  });

  test("launcher: stable after settle (regression baseline)", async () => {
    // Второй snapshot launcher'а после большего settle window — для уверенности
    // что между двумя независимыми launch'ами картинка детерминирована.
    const app = await launchKepler({ slug: "visual-launcher-settled" });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);
      await launcher.evaluate(() => document.fonts?.ready ?? Promise.resolve());
      await launcher.waitForTimeout(1500);
      await expect(launcher).toHaveScreenshot("launcher-settled.png", {
        ...SNAP_OPTS,
        timeout: 10_000,
      });
    } finally {
      await app.close();
    }
  });
});
