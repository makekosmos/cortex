// Deterministic wait helpers для e2e тестов.
//
// Заменяют ad-hoc `await page.waitForTimeout(2500)` на конкретные сигналы от
// приложения — backend handshake, command registration, etc. См.
// .agent/tasks/2026-05-21-bug-detection-phase1-determinism/.
//
// Все хелперы предполагают что Kepler shell запущен с `KOSMOS_TEST_MODE=1`,
// тогда preload exposes `window.kepler.__test`. См. platform/desktop/electron/preload.ts.

import type { Page } from "playwright";

/**
 * Ждёт пока ArkClient в main process отрезолвится (handshake с
 * kepler-backend done). Резолвится сразу если уже ready.
 *
 * Использовать вместо `await page.waitForTimeout(2500)` после launch.
 */
export async function waitForBackendReady(page: Page, timeoutMs = 15_000): Promise<void> {
  await page.waitForFunction(
    () =>
      typeof (window as unknown as { kepler?: { __test?: unknown } }).kepler?.__test !==
      "undefined",
    null,
    { timeout: 5_000 },
  );
  await page.evaluate(async (ms: number) => {
    const k = (
      window as unknown as {
        kepler: {
          __test: { waitForReady(timeoutMs: number): Promise<void> };
        };
      }
    ).kepler;
    await k.__test.waitForReady(ms);
  }, timeoutMs);
}
