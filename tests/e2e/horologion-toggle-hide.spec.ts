// Horologion mode-toggle hide-when-session-active regression.
//
// User report: при старте секундомера/помодоро в шайбе остаётся надпись
// того режима, который не включён. Раньше неактивный toggle сжимался
// в 0 (CSS `.mode-toggle__btn--hidden`). После commit'а 877781f это
// поведение временно убрано из-за orphan time_entries. С af437e5
// (orphan filter в shim) `isSessionActive` точный → restore hide.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

test("horologion: при активной сессии inactive toggle получает --hidden", async () => {
  const app = await launchKepler({ slug: "horologion-toggle-hide" });
  try {
    await new Promise((r) => setTimeout(r, 2000));
    await app.evaluate(async ({ BrowserWindow }, cid) => {
      const launcher = BrowserWindow.getAllWindows()[0];
      if (!launcher) throw new Error("no launcher");
      await launcher.webContents.executeJavaScript(
        `window.kepler?.commands?.invoke?.(${JSON.stringify(cid)})`,
      );
    }, "horologion:open");

    const horo = await app.waitForEvent("window", { timeout: 10_000 });
    await horo.waitForLoadState("domcontentloaded");
    await horo.waitForTimeout(1500);

    // Initial — оба toggle видимы (не имеют --hidden).
    const pomoBtn = horo.getByRole("tab", { name: "Помодоро" });
    const swBtn = horo.getByRole("tab", { name: "Секундомер" });
    await expect(pomoBtn).not.toHaveClass(/mode-toggle__btn--hidden/);
    await expect(swBtn).not.toHaveClass(/mode-toggle__btn--hidden/);

    // Переключаемся на Секундомер, стартуем сессию.
    await swBtn.click();
    await horo.waitForTimeout(600);
    // StopwatchView's primary button (класс .sw__primary) — однозначно.
    await horo.locator(".sw__primary").click();
    await horo.waitForTimeout(1500);

    // Pomodoro toggle ДОЛЖЕН быть hidden (--hidden class).
    await expect(pomoBtn).toHaveClass(/mode-toggle__btn--hidden/, {
      timeout: 3_000,
    });
    // Stopwatch toggle (active) — НЕ hidden.
    await expect(swBtn).not.toHaveClass(/mode-toggle__btn--hidden/);

    // Стоп сессии — primary button у StopwatchView в running state
    // (тот же `.sw__primary` элемент, текст «Стоп» через v-if).
    await horo.locator(".sw__primary").click();
    await horo.waitForTimeout(800);

    // Оба toggle снова видимы.
    await expect(pomoBtn).not.toHaveClass(/mode-toggle__btn--hidden/, {
      timeout: 3_000,
    });
    await expect(swBtn).not.toHaveClass(/mode-toggle__btn--hidden/);
  } finally {
    await app.evaluate(({ app: a }) => a.quit());
    await Promise.race([
      new Promise<void>((r) => app.process().once("exit", () => r())),
      new Promise<void>((_, rej) => setTimeout(() => rej(new Error("exit timeout 10s")), 10_000)),
    ]);
  }
});
