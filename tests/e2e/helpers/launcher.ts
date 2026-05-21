// Помощники для работы с launcher window (mainWindow) в e2e тестах.

import { expect, type Page } from "@playwright/test";
import type { ElectronApplication } from "playwright";

/**
 * Возвращает первое окно — launcher.
 *
 * В тестах `createLauncher` зовётся в whenReady с `show: false`, и Playwright
 * `firstWindow()` всё равно отдаёт его (webContents существует, видимость
 * не важна для DOM-операций).
 */
export async function getLauncherWindow(app: ElectronApplication): Promise<Page> {
  const launcher = await app.firstWindow();
  await launcher.waitForLoadState("domcontentloaded");
  return launcher;
}

/**
 * Ждёт появления update-tile в нужном состоянии.
 * Использует CSS-классы из `LauncherView.vue`.
 */
export async function waitForUpdateTile(
  launcher: Page,
  kind: "downloading" | "downloaded" | "available",
): Promise<void> {
  const tile = launcher.locator(".update-tile").first();
  await expect(tile).toBeVisible({ timeout: 5_000 });
  if (kind === "downloading") {
    await expect(tile.locator(".update-tile-progress")).toBeVisible({
      timeout: 5_000,
    });
  } else {
    // Для downloaded / available — fill-элемент НЕ рендерится
    // (updateBanner.progress === undefined).
    await expect(tile.locator(".update-tile-progress")).toHaveCount(0);
  }
}

/**
 * Ждёт появления post-update tile (показывается только после kepler:post-update event'а).
 */
export async function waitForPostUpdateTile(launcher: Page): Promise<void> {
  await expect(launcher.locator(".post-update-tile")).toBeVisible({
    timeout: 5_000,
  });
}
