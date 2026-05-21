// Horologion pomodoro flow (AC4).
//
// Сценарий:
//   1. Открываем Horologion, режим Pomodoro по умолчанию.
//   2. Initial time: workMin:00 (default 25:00).
//   3. Кликаем «Начать сессию» → phase=work, status=Идёт…
//   4. Ждём 1.5s → time decreased.
//   5. Кликаем «Пауза» → status=Пауза, время заморожено.
//   6. Кликаем «Продолжить» → status=Идёт… снова.
//   7. Кликаем «Стоп» → phase=idle, time = workMin:00.
//   8. Никаких missing field errors.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

test("horologion: pomodoro start → pause → resume → stop", async () => {
  const app = await launchKepler({ slug: "horologion-pomodoro-flow" });
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

    // Pomodoro по дефолту активен.
    const timeLabel = horoWindow.locator(".pomo__time");
    const initialTime = (await timeLabel.textContent()) ?? "";
    expect(initialTime).toMatch(/^\d{2}:00$/);
    const initialMinutes = parseInt(initialTime.split(":")[0]!, 10);
    expect(initialMinutes).toBeGreaterThan(0);

    // Start.
    const startBtn = horoWindow.getByRole("button", { name: /Начать сессию/ });
    await expect(startBtn).toBeVisible({ timeout: 3_000 });
    await startBtn.click();
    await horoWindow.waitForTimeout(1500);

    // Phase = work, status = "Идёт…".
    await expect(horoWindow.locator(".pomo__phase")).toHaveText("Фокус", {
      timeout: 3_000,
    });
    await expect(horoWindow.locator(".pomo__status")).toHaveText("Идёт…");

    const runningTime = (await timeLabel.textContent()) ?? "";
    // Time decreased (or hit MM:5x range).
    expect(runningTime).not.toBe(initialTime);

    // Pause.
    const pauseBtn = horoWindow.getByRole("button", { name: /^Пауза/ });
    await expect(pauseBtn).toBeVisible({ timeout: 3_000 });
    await pauseBtn.click();
    await horoWindow.waitForTimeout(500);

    await expect(horoWindow.locator(".pomo__status")).toHaveText("Пауза");
    const pausedTime = (await timeLabel.textContent()) ?? "";

    // Wait 1s — time should NOT decrease while paused.
    await horoWindow.waitForTimeout(1100);
    const stillPausedTime = (await timeLabel.textContent()) ?? "";
    expect(stillPausedTime).toBe(pausedTime);

    // Resume.
    const resumeBtn = horoWindow.getByRole("button", { name: /Продолжить/ });
    await expect(resumeBtn).toBeVisible();
    await resumeBtn.click();
    await horoWindow.waitForTimeout(1500);

    await expect(horoWindow.locator(".pomo__status")).toHaveText("Идёт…");
    const resumedTime = (await timeLabel.textContent()) ?? "";
    expect(resumedTime).not.toBe(pausedTime);

    // Stop.
    const stopBtn = horoWindow.getByRole("button", { name: /^Стоп/ });
    await expect(stopBtn).toBeVisible({ timeout: 3_000 });
    await stopBtn.click();
    await horoWindow.waitForTimeout(800);

    // Back to idle.
    await expect(timeLabel).toHaveText(initialTime, { timeout: 3_000 });
    await expect(horoWindow.getByRole("button", { name: /Начать сессию/ })).toBeVisible();

    // No missing field errors.
    const missingFieldErrors = consoleErrors.filter((e) => e.includes("missing field"));
    if (missingFieldErrors.length > 0) {
      throw new Error(
        `${missingFieldErrors.length} «missing field» errors:\n${missingFieldErrors.join("\n")}`,
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
