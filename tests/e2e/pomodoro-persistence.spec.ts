// AC9-AC11: pomodoro session переживает renderer close+reopen.
//
// Сценарий:
//   1. Открыть Kepler + Horologion.
//   2. Начать pomodoro (start).
//   3. Дождаться, что таймер тикает (remainingMs уменьшился).
//   4. Закрыть окно Horologion (не quit'ить app, только window).
//   5. Подождать ~3-5s — backend всё тикает.
//   6. Открыть Horologion снова через command bus.
//   7. Verify: timer показывает время МЕНЬШЕ изначального startMin*60
//      на (sum of waits) ±допустимая погрешность.
//   8. Остановить pomodoro для cleanup.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

function parseMmSs(s: string): number {
  const m = /^(\d{2}):(\d{2})$/.exec(s);
  if (!m) return NaN;
  return parseInt(m[1]!, 10) * 60 + parseInt(m[2]!, 10);
}

test("pomodoro: session переживает renderer close+reopen", async () => {
  const app = await launchKepler({ slug: "pomodoro-persistence" });

  try {
    await new Promise((r) => setTimeout(r, 2000));

    async function openHorologion() {
      await app.evaluate(async ({ BrowserWindow }, commandId) => {
        const wins = BrowserWindow.getAllWindows();
        const launcher = wins[0];
        if (!launcher) throw new Error("no launcher window");
        await launcher.webContents.executeJavaScript(
          `window.kepler?.commands?.invoke?.(${JSON.stringify(commandId)})`,
        );
      }, "horologion:open");
      return app.waitForEvent("window", { timeout: 10_000 });
    }

    // 1. Open Horologion.
    let horoWindow = await openHorologion();
    await horoWindow.waitForLoadState("domcontentloaded");
    await horoWindow.waitForTimeout(1500);

    // Дефолт — pomodoro view; стартуем.
    const startBtn = horoWindow.getByRole("button", { name: /Начать сессию/ });
    await expect(startBtn).toBeVisible({ timeout: 5_000 });
    await startBtn.click();
    await horoWindow.waitForTimeout(2000);

    // Должен быть таймер running.
    const timeLabel1 = horoWindow.locator(".pomo__time");
    await expect(horoWindow.locator(".pomo__phase")).toHaveText("Фокус", {
      timeout: 5_000,
    });
    const beforeCloseText = (await timeLabel1.textContent()) ?? "";
    const beforeCloseSec = parseMmSs(beforeCloseText);
    expect(beforeCloseSec).toBeGreaterThan(0);

    // 2. Закрыть окно Horologion.
    const horoWebContentsId = await horoWindow.evaluate(() => {
      return undefined; // We need main process side
    });
    void horoWebContentsId;
    await app.evaluate(({ BrowserWindow }) => {
      const wins = BrowserWindow.getAllWindows();
      const horo = wins.find(
        (w) => w.getTitle().toLowerCase().includes("horologion") ||
              w.webContents.getURL().includes("horologion"),
      );
      if (horo) horo.close();
    });

    // 3. Подождать ~4 секунды — backend всё тикает.
    await new Promise((r) => setTimeout(r, 4000));

    // 4. Открыть Horologion снова.
    horoWindow = await openHorologion();
    await horoWindow.waitForLoadState("domcontentloaded");
    await horoWindow.waitForTimeout(2000);

    // Timer должен показывать `Фокус` фазу (session не сброшена).
    await expect(horoWindow.locator(".pomo__phase")).toHaveText("Фокус", {
      timeout: 5_000,
    });
    const timeLabel2 = horoWindow.locator(".pomo__time");
    const afterReopenText = (await timeLabel2.textContent()) ?? "";
    const afterReopenSec = parseMmSs(afterReopenText);
    expect(afterReopenSec).toBeGreaterThan(0);

    // Между close и reopen прошло ~4 секунды + 2 для reload + 2 init.
    // Должно быть строго МЕНЬШЕ значения до close хотя бы на 3 секунды.
    // (Если значение НЕ уменьшилось — state был сброшен.)
    expect(afterReopenSec).toBeLessThan(beforeCloseSec);
    expect(beforeCloseSec - afterReopenSec).toBeGreaterThanOrEqual(3);

    // Cleanup — стоп.
    const stopBtn = horoWindow.getByRole("button", { name: /^Стоп/ });
    await expect(stopBtn).toBeVisible({ timeout: 3_000 });
    await stopBtn.click();
    await horoWindow.waitForTimeout(500);
  } finally {
    await app.evaluate(({ app: a }) => a.quit());
    await Promise.race([
      new Promise<void>((resolve) =>
        app.process().once("exit", () => resolve()),
      ),
      new Promise<void>((_, rej) =>
        setTimeout(() => rej(new Error("process exit timeout 10s")), 10_000),
      ),
    ]);
  }
});
