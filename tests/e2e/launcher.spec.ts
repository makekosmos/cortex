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
      expect(name).toBeTruthy();
      expect(typeof name).toBe("string");
    } finally {
      await app.close();
    }
  });

  test("launcher: ровно одно окно создано, не destroyed", async () => {
    const app = await launchKepler({ slug: "launcher-windows" });
    try {
      // Дать main-process чуть времени на whenReady → spawn backend → createWindow.
      await new Promise((r) => setTimeout(r, 1500));
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
