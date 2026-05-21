// Horologion stopwatch full flow (AC3 + AC5 — missing field id fix).
//
// Сценарий:
//   1. Открываем Horologion через command bus.
//   2. Переключаемся на «Секундомер» mode.
//   3. Кликаем «Начать сессию» → таймер запускается.
//   4. Ждём 2.5s → таймер показывает >= 00:00:02.
//   5. Кликаем «Стоп» → таймер сбрасывается, кнопка вернулась в «Начать».
//   6. Никаких console errors `missing field 'id'` в логах.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

test("horologion: stopwatch start → tick → stop", async () => {
  const app = await launchKepler({ slug: "horologion-stopwatch-flow" });
  const consoleErrors: string[] = [];
  app.on("window", (w) => {
    w.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });
    w.on("pageerror", (e) => consoleErrors.push(`pageerror: ${e.message}`));
  });

  try {
    await new Promise((r) => setTimeout(r, 2000));

    await app.evaluate(async ({ BrowserWindow }, commandId) => {
      const wins = BrowserWindow.getAllWindows();
      const launcher = wins[0];
      if (!launcher) throw new Error("no launcher window");
      await launcher.webContents.executeJavaScript(
        `window.kepler?.commands?.invoke?.(${JSON.stringify(commandId)})`,
      );
    }, "horologion:open");

    const horoWindow = await app.waitForEvent("window", { timeout: 10_000 });
    await horoWindow.waitForLoadState("domcontentloaded");
    await horoWindow.waitForTimeout(1500);

    // Кликаем на «Секундомер» toggle button.
    const stopwatchToggle = horoWindow.getByRole("tab", {
      name: "Секундомер",
    });
    await stopwatchToggle.click();
    // Vue <transition> между PomodoroView ↔ StopwatchView держит обе view'хи
    // в DOM на время анимации. Ждём пока pomo__primary detach'нется —
    // иначе getByRole("button", { name: /Начать сессию/ }) находит обе кнопки
    // (pomo + sw) и strict-mode fails.
    await horoWindow.waitForSelector(".pomo__primary", { state: "detached", timeout: 3_000 });

    // Initial state: timer = 00:00:00, primary button = «Начать сессию».
    const startBtn = horoWindow.locator(".sw__primary");
    await expect(startBtn).toBeVisible({ timeout: 3_000 });
    await expect(startBtn).toHaveText(/Начать сессию/);

    const timerText = horoWindow.locator(".sw__time");
    await expect(timerText).toHaveText("00:00:00");

    // Старт.
    await startBtn.click();
    await horoWindow.waitForTimeout(2_500);

    // После 2.5s timer должен показывать >= 00:00:02.
    const afterStart = (await timerText.textContent()) ?? "";
    expect(afterStart).not.toBe("00:00:00");
    // Парсим HH:MM:SS → seconds.
    const m = afterStart.match(/^(\d{2}):(\d{2}):(\d{2})$/);
    expect(m).not.toBeNull();
    if (m) {
      const secs = Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]);
      expect(secs).toBeGreaterThanOrEqual(2);
    }

    // Кнопка теперь «Стоп».
    const stopBtn = horoWindow.locator(".sw__primary");
    await expect(stopBtn).toBeVisible({ timeout: 3_000 });
    await expect(stopBtn).toHaveText(/Стоп/);

    // Стоп.
    await stopBtn.click();
    await horoWindow.waitForTimeout(800);

    // Timer reset, кнопка снова «Начать сессию».
    await expect(timerText).toHaveText("00:00:00", { timeout: 3_000 });
    await expect(startBtn).toHaveText(/Начать сессию/);

    // No `missing field id` errors throughout.
    const missingFieldErrors = consoleErrors.filter((e) => e.includes("missing field"));
    if (missingFieldErrors.length > 0) {
      throw new Error(
        `${missingFieldErrors.length} «missing field» errors detected:\n${missingFieldErrors.join("\n")}`,
      );
    }
  } finally {
    await app.evaluate(({ app: a }) => a.quit());
    await Promise.race([
      new Promise<void>((resolve) => app.process().once("exit", () => resolve())),
      new Promise<void>((_, rej) =>
        setTimeout(() => rej(new Error("process exit timeout 10s")), 10_000),
      ),
    ]);
  }
});
