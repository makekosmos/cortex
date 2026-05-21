// Делphi extension smoke (AC1 / AC10).
//
// Открывает Делphi extension window через IPC `kepler:extension:open`,
// ждёт first new window (extension живёт в отдельном BrowserWindow),
// проверяет sidebar items.
//
// Если sidebar items НЕ найдены — fail с подробным сообщением, включая
// HTML snippet и список реально найденных пунктов.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

const REQUIRED_SIDEBAR_ITEMS = ["Входящие", "Сегодня", "Журнал", "Корзина", "Проекты"];

test.describe("delphi extension", () => {
  test("delphi sidebar содержит все обязательные пункты", async () => {
    const app = await launchKepler({ slug: "delphi-sidebar" });
    try {
      // Дать main-process подняться (whenReady + backend spawn).
      await new Promise((r) => setTimeout(r, 2000));

      // Открываем Делphi extension через main-process API.
      await app.evaluate(async ({ ipcMain }, _payload) => {
        const { Promise: P } = globalThis;
        return new P((resolve) => setTimeout(resolve, 0));
      });

      // Триггерим открытие Делphi через зарегистрированную command bus
      // команду `delphi:open` (см. shell/electron/commands.ts).
      const opened = await app.evaluate(async ({ BrowserWindow }, commandId) => {
        const wins = BrowserWindow.getAllWindows();
        const launcher = wins[0];
        if (!launcher) return { triggered: false, before: 0 };
        await launcher.webContents.executeJavaScript(
          `window.kepler?.commands?.invoke?.(${JSON.stringify(commandId)})`,
        );
        return { triggered: true, before: wins.length };
      }, "delphi:open");

      // Ждём появления второго окна (Delphi).
      const delphiWindow = await app.waitForEvent("window", { timeout: 10_000 });
      await delphiWindow.waitForLoadState("domcontentloaded");

      // Дать Vue смонтироваться.
      await delphiWindow.waitForTimeout(1500);

      // Собираем текст всего sidebar'а (или fallback: всего body).
      const sidebarText = await delphiWindow
        .locator("[data-testid=sidebar], aside, nav")
        .first()
        .textContent({ timeout: 5_000 })
        .catch(() => null);

      const bodyText = sidebarText ?? (await delphiWindow.locator("body").textContent());

      const missing = REQUIRED_SIDEBAR_ITEMS.filter((item) => !bodyText?.includes(item));
      if (missing.length > 0) {
        const found = REQUIRED_SIDEBAR_ITEMS.filter((i) => bodyText?.includes(i));
        throw new Error(
          `Делphi sidebar НЕ содержит обязательные пункты: [${missing.join(", ")}]. ` +
            `Найдены: [${found.join(", ")}]. Sidebar text (200 chars): ` +
            `${(bodyText ?? "").slice(0, 200)}`,
        );
      }

      for (const item of REQUIRED_SIDEBAR_ITEMS) {
        expect(bodyText).toContain(item);
      }
      // opened для guard на unused переменную
      expect(opened.triggered).toBe(true);
    } finally {
      await app.close();
    }
  });
});
