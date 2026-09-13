import type { Page } from "playwright";
export type BackendStats = {
  arkConnected?: boolean;
  commandsRegistered?: number;
  commands?: string[];
};
export function hasLiveBackend(stats: BackendStats): boolean {
  return Boolean(
    stats.arkConnected &&
    Array.isArray(stats.commands) &&
    stats.commandsRegistered === stats.commands.length &&
    stats.commands.includes("settings:open"),
  );
}
export async function waitForBackendReady(page: Page, timeout = 30_000): Promise<void> {
  await page.waitForFunction(
    async () => {
      const test = window.kepler?.__test;
      if (!test) return false;
      try {
        await test.waitForReady(1_000);
        const stats = await test.getStats();
        return Boolean(
          stats.arkConnected &&
          Array.isArray(stats.commands) &&
          stats.commandsRegistered === stats.commands.length &&
          stats.commands.includes("settings:open"),
        );
      } catch {
        return false;
      }
    },
    null,
    { timeout },
  );
}
