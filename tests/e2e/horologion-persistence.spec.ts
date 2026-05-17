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
    // Vue <transition> между Pomodoro/Stopwatch view'хами держит оба элемента
    // в DOM на время анимации (~300ms). Ждём до момента когда .pomo__primary
    // ушёл — иначе getByRole("button", { name: /Начать сессию/ }) находит 2
    // элемента (pomo + sw) и strict-mode fails.
    await horoWindow.waitForSelector(".pomo__primary", { state: "detached", timeout: 3_000 });

    const startBtn = horoWindow.locator(".sw__primary");
    await expect(startBtn).toBeVisible({ timeout: 5_000 });
    await startBtn.click();
    await horoWindow.waitForTimeout(2_000);

    const stopBtn = horoWindow.locator(".sw__primary"); // тот же button, текст «Стоп»
    await expect(stopBtn).toBeVisible({ timeout: 5_000 });
    await expect(stopBtn).toHaveText(/Стоп/);
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

test("horologion: переключение режимов меняет selected tab", async () => {
  // Проверяет UX переключения режимов через role=tab + aria-selected. Раньше
  // отдельно проверяли visible-текст «помодоро»/«секундомер» через .pomo__hint
  // / .sw__hint label'ы; от .pomo__hint отказались сознательно (избыточный
  // дублирующий label при наличии явного активного tab'а), поэтому
  // тест сейчас опирается на семантику ARIA — это всё равно правильный
  // user-facing inv.
  const app = await launchKepler({ slug: "horologion-mode-hint-visible" });
  try {
    const horoWindow = await openHorologion(app);

    const pomoTab = horoWindow.getByRole("tab", { name: "Помодоро" });
    const swTab = horoWindow.getByRole("tab", { name: "Секундомер" });

    // Default mode = pomodoro.
    await expect(pomoTab).toHaveAttribute("aria-selected", "true", { timeout: 5_000 });
    await expect(swTab).toHaveAttribute("aria-selected", "false");

    // Switch to stopwatch.
    await swTab.click();
    await horoWindow.waitForTimeout(800);
    await expect(swTab).toHaveAttribute("aria-selected", "true");
    await expect(pomoTab).toHaveAttribute("aria-selected", "false");
    // Stopwatch keeps its own identity label «секундомер» (sw__hint),
    // pomodoro намеренно без аналога (см. выше).
    await expect(horoWindow.locator(".sw__hint")).toBeVisible();

    // Switch back.
    await pomoTab.click();
    await horoWindow.waitForTimeout(800);
    await expect(pomoTab).toHaveAttribute("aria-selected", "true");
    await expect(swTab).toHaveAttribute("aria-selected", "false");
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
