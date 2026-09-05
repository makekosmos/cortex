import type { Page } from "playwright";
export async function waitForBackendReady(page: Page, timeout = 30_000): Promise<void> {
  await page.waitForFunction(() => Boolean(window.kepler?.ark), null, { timeout });
}
