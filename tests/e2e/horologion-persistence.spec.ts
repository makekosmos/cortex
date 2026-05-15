// Horologion persistence — pomodoro / stopwatch создают видимую запись в
// ListView после остановки.
//
// Эти тесты воспроизводят user-bug post-Phase-4: «stop pomodoro → снизу
// нет записи». А также проверяют что stopwatch start → stop оставляет
// видимую запись.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

async function openHorologion(app: Awaited<ReturnType<typeof launchKepler>>) {
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
  return horoWindow;
}

test("horologion: pomodoro start → stop → entry visible in ListView", async () => {
  const app = await launchKepler({ slug: "horologion-pomodoro-persistence" });
  try {
    const horoWindow = await openHorologion(app);

    // Set pomodoro draft title via input so we can identify the entry later.
    const draftInput = horoWindow.locator(".pdi__row input, .pdi__row textarea, .pdi__row [contenteditable]").first();
    await draftInput.click();
    await horoWindow.keyboard.type("e2e-pomo-task");

    // Start pomodoro.
    const startBtn = horoWindow.getByRole("button", { name: /Начать сессию/ });
    await expect(startBtn).toBeVisible({ timeout: 5_000 });
    await startBtn.click();
    await horoWindow.waitForTimeout(2_000);

    // Stop pomodoro via "Стоп" secondary button.
    const stopBtn = horoWindow.getByRole("button", { name: /^Стоп/ });
    await expect(stopBtn).toBeVisible({ timeout: 5_000 });
    await stopBtn.click();
    await horoWindow.waitForTimeout(1_500);

    // Entry must be visible in ListView.
    const listBody = horoWindow.locator(".list");
    await expect(listBody).toBeVisible();

    // Either days группа с entries, либо empty state. Не должно быть empty.
    const empty = horoWindow.locator(".empty");
    const emptyVisible = await empty.isVisible().catch(() => false);
    expect(emptyVisible).toBe(false);

    // Найдём row с title содержащим наш draft.
    const row = horoWindow.locator(".row__title", { hasText: "e2e-pomo-task" });
    await expect(row).toBeVisible({ timeout: 3_000 });
  } finally {
    await app.evaluate(({ app: a }) => a.quit());
    await Promise.race([
      new Promise<void>((resolve) =>
        app.process().once("exit", () => resolve()),
      ),
      new Promise<void>((_, rej) =>
        setTimeout(() => rej(new Error("process exit timeout")), 10_000),
      ),
    ]);
  }
});

test("horologion: stopwatch start → stop → entry visible in ListView", async () => {
  const app = await launchKepler({ slug: "horologion-stopwatch-persistence" });
  try {
    const horoWindow = await openHorologion(app);

    // Set draft title.
    const draftInput = horoWindow.locator(".pdi__row input, .pdi__row textarea, .pdi__row [contenteditable]").first();
    await draftInput.click();
    await horoWindow.keyboard.type("e2e-sw-task");

    // Switch to stopwatch.
    await horoWindow.getByRole("tab", { name: "Секундомер" }).click();
    await horoWindow.waitForTimeout(600);

    const startBtn = horoWindow.getByRole("button", { name: /Начать сессию/ });
    await expect(startBtn).toBeVisible({ timeout: 5_000 });
    await startBtn.click();
    await horoWindow.waitForTimeout(2_000);

    const stopBtn = horoWindow.getByRole("button", { name: /Стоп/ });
    await expect(stopBtn).toBeVisible({ timeout: 5_000 });
    await stopBtn.click();
    await horoWindow.waitForTimeout(1_200);

    const empty = horoWindow.locator(".empty");
    const emptyVisible = await empty.isVisible().catch(() => false);
    expect(emptyVisible).toBe(false);

    const row = horoWindow.locator(".row__title", { hasText: "e2e-sw-task" });
    await expect(row).toBeVisible({ timeout: 3_000 });
  } finally {
    await app.evaluate(({ app: a }) => a.quit());
    await Promise.race([
      new Promise<void>((resolve) =>
        app.process().once("exit", () => resolve()),
      ),
      new Promise<void>((_, rej) =>
        setTimeout(() => rej(new Error("process exit timeout")), 10_000),
      ),
    ]);
  }
});

test("horologion: подпись режима видна и в pomodoro и в stopwatch", async () => {
  // H2: воспроизводит user-bug «подпись секундомер исчезает в pomodoro mode».
  // Fix: PomodoroView получил свою подпись «помодоро» по аналогии со
  // StopwatchView'овским «секундомер». Юзер всегда видит current mode label.
  const app = await launchKepler({ slug: "horologion-mode-hint-visible" });
  try {
    const horoWindow = await openHorologion(app);

    // Default mode = pomodoro → должна быть подпись «помодоро».
    await expect(horoWindow.locator(".pomo__hint")).toBeVisible({ timeout: 5_000 });
    await expect(horoWindow.locator(".pomo__hint")).toHaveText("помодоро");

    // Switch to stopwatch.
    await horoWindow.getByRole("tab", { name: "Секундомер" }).click();
    await horoWindow.waitForTimeout(1500);
    await expect(horoWindow.locator(".sw__hint")).toBeVisible();
    await expect(horoWindow.locator(".sw__hint")).toHaveText("секундомер");

    // Switch back to pomodoro — подпись «помодоро» снова видна.
    await horoWindow.getByRole("tab", { name: "Помодоро" }).click();
    await horoWindow.waitForTimeout(1500);
    await expect(horoWindow.locator(".pomo__hint")).toBeVisible();
    await expect(horoWindow.locator(".pomo__hint")).toHaveText("помодоро");
  } finally {
    await app.evaluate(({ app: a }) => a.quit());
    await Promise.race([
      new Promise<void>((resolve) =>
        app.process().once("exit", () => resolve()),
      ),
      new Promise<void>((_, rej) =>
        setTimeout(() => rej(new Error("process exit timeout")), 10_000),
      ),
    ]);
  }
});
