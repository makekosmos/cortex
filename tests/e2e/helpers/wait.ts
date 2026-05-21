// Deterministic wait helpers для e2e тестов.
//
// Заменяют ad-hoc `await page.waitForTimeout(2500)` на конкретные сигналы от
// приложения — backend handshake, command registration, etc. См.
// .agent/tasks/2026-05-21-bug-detection-phase1-determinism/.
//
// Все хелперы предполагают что Kepler shell запущен с `KOSMOS_TEST_MODE=1`,
// тогда preload exposes `window.kepler.__test`. См. shell/electron/preload.ts.

import type { Page } from "playwright";

/**
 * Ждёт пока ArkClient в main process отрезолвится (handshake с
 * kepler-backend done). Резолвится сразу если уже ready.
 *
 * Использовать вместо `await page.waitForTimeout(2500)` после launch.
 */
export async function waitForBackendReady(
  page: Page,
  timeoutMs = 15_000,
): Promise<void> {
  await page.waitForFunction(
    () =>
      typeof (window as unknown as { kepler?: { __test?: unknown } }).kepler
        ?.__test !== "undefined",
    null,
    { timeout: 5_000 },
  );
  await page.evaluate(
    async (ms: number) => {
      const k = (window as unknown as {
        kepler: {
          __test: { waitForReady(timeoutMs: number): Promise<void> };
        };
      }).kepler;
      await k.__test.waitForReady(ms);
    },
    timeoutMs,
  );
}

/**
 * Ждёт пока command с указанным id появится в registry (kepler-internal,
 * manifest-declared или runtime-registered). Используется когда тест
 * открывает extension и ему нужно дождаться, что команды extension'а
 * прорегистрированы до того как делать assertions.
 */
export async function waitForCommandRegistered(
  page: Page,
  commandId: string,
  timeoutMs = 5_000,
): Promise<void> {
  await page.waitForFunction(
    async (id: string) => {
      const k = (window as unknown as {
        kepler?: {
          __test?: {
            getStats(): Promise<{ commands: string[] }>;
          };
        };
      }).kepler;
      if (!k?.__test) return false;
      const stats = await k.__test.getStats();
      return stats.commands.includes(id);
    },
    commandId,
    { timeout: timeoutMs, polling: 100 },
  );
}

/**
 * Snapshot текущей статистики (для assertions). Throws если test rig не
 * exposed (KOSMOS_TEST_MODE не выставлен).
 */
export async function getTestStats(page: Page): Promise<{
  arkConnected: boolean;
  commands: string[];
  commandsRegistered: number;
}> {
  return page.evaluate(async () => {
    const k = (window as unknown as {
      kepler?: {
        __test?: {
          getStats(): Promise<{
            arkConnected: boolean;
            commands: string[];
            commandsRegistered: number;
          }>;
        };
      };
    }).kepler;
    if (!k?.__test) {
      throw new Error(
        "window.kepler.__test не exposed — KOSMOS_TEST_MODE=1 не выставлен?",
      );
    }
    return k.__test.getStats();
  });
}
