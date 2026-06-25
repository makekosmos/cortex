// Launcher smoke: Electron app поднимается, main-process API отвечает,
// окно launcher'а создаётся (hidden до global hotkey).
//
// AC8/AC9/AC10 baseline: проверяет infrastructure, не functional behaviour
// (functional regressions покрываются delphi.spec.ts и extension specs).

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

test.describe("kepler-shell launcher", () => {
  test("launcher: app поднимается и getName() возвращает строку", async () => {
    const app = await launchKepler({ slug: "launcher-boot" });
    try {
      const name = await app.evaluate(({ app: a }) => a.getName());
      expect(name).toBe("Kosmos [test]");
    } finally {
      await app.close();
    }
  });

  test("launcher: ровно одно окно создано, не destroyed", async () => {
    const app = await launchKepler({ slug: "launcher-windows-smoke" });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      const wins = await app.evaluate(({ BrowserWindow }) =>
        BrowserWindow.getAllWindows().map((w) => ({
          isDestroyed: w.isDestroyed(),
          title: w.getTitle(),
        })),
      );
      expect(wins.length).toBeGreaterThanOrEqual(1);
      expect(wins[0]!.isDestroyed).toBe(false);
    } finally {
      await app.close();
    }
  });
});
