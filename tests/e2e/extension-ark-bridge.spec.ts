// Extension ARK bridge ready-gate regression.
//
// Race-condition: extension window открывается раньше, чем main.ts успевает
// вызвать `setExtensionArkBridge(...)` (это происходит после handshake'а
// ArkClient'а). Первый probe из extension'а в этот момент летел в handler
// `kepler:extension:ark:request`, который немедленно throw'ил
// "ark bridge not ready" → extension UI запоминал status=error и держал
// его до следующего probe-интервала (10s).
//
// После fix'а handler ждёт ready-promise (как kepler:ark:request) → первый
// запрос не fail'ится, indicator корректно становится «connected».
//
// Этот тест открывает Eden СРАЗУ через command bus (без 2s warm-up),
// захватывает console errors из extension renderer'а и проверяет, что
// никаких «ark bridge not ready» в первые секунды нет.

import { test, expect, type ConsoleMessage, type Page } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

type KeplerTestApp = Awaited<ReturnType<typeof launchKepler>>;

const INVOKE_EDEN_OPEN_SCRIPT = `
  (async () => {
    if (typeof window.kepler?.commands?.invoke !== "function") {
      return "no-api";
    }
    try {
      await window.kepler.commands.invoke("eden:open");
      return "ok";
    } catch (e) {
      return "throw:" + (e && e.message ? e.message : String(e));
    }
  })()
`;

function attachPageErrorCollectors(
  page: Page,
  consoleErrors: string[],
  pageErrors: string[],
): void {
  page.on("console", (msg: ConsoleMessage) => {
    if (msg.type() === "error") {
      consoleErrors.push(msg.text());
    }
  });
  page.on("pageerror", (err: Error) => {
    pageErrors.push(err.message);
  });
}

function collectFuturePageErrors(app: KeplerTestApp): {
  consoleErrors: string[];
  pageErrors: string[];
} {
  const consoleErrors: string[] = [];
  const pageErrors: string[] = [];
  app.on("window", (page: Page) => {
    attachPageErrorCollectors(page, consoleErrors, pageErrors);
  });
  return { consoleErrors, pageErrors };
}

async function invokeEdenOpenFromLauncher(app: KeplerTestApp): Promise<void> {
  await expect
    .poll(
      () =>
        app.evaluate(async ({ BrowserWindow }, script) => {
          const wins = BrowserWindow.getAllWindows();
          const launcher = wins[0];
          if (!launcher) return "no-launcher";
          return (await launcher.webContents.executeJavaScript(script)) as string;
        }, INVOKE_EDEN_OPEN_SCRIPT),
      {
        intervals: [100],
        timeout: 3_000,
        message: "commands.invoke должен стать доступен в launcher window",
      },
    )
    .toBe("ok");
}

async function waitForExtensionArkBridge(extensionWindow: Page): Promise<void> {
  await expect
    .poll(
      () =>
        extensionWindow.evaluate(async () => {
          const kepler = (
            window as unknown as {
              kepler?: {
                ark?: { request(operation: string, params?: unknown): Promise<unknown> };
              };
            }
          ).kepler;
          if (typeof kepler?.ark?.request !== "function") return "no-api";
          try {
            await kepler.ark.request("list_objects");
            return "ok";
          } catch (e) {
            return "throw:" + (e && e instanceof Error ? e.message : String(e));
          }
        }),
      {
        intervals: [100, 250, 500],
        timeout: 5_000,
        message: "extension ARK bridge должен принять первый request",
      },
    )
    .toBe("ok");
}

test.describe("extension ark bridge", () => {
  test("первый extension probe не fails — bridge ready-gate работает", async () => {
    const app = await launchKepler({ slug: "extension-ark-bridge" });
    try {
      // Минимальный wait — только дождаться, что launcher window вообще
      // загрузился и preload exposed window.kepler. Никаких 2s sleep'ов:
      // мы специально хотим race с initArkClient.
      const launcherPage = await app.firstWindow();
      await launcherPage.waitForLoadState("domcontentloaded");

      // Собираем все будущие extension page errors. Setup ДО открытия —
      // чтобы не упустить ранние логи.
      const { consoleErrors, pageErrors } = collectFuturePageErrors(app);

      // Триггерим открытие Eden как можно раньше — через invoke
      // open-команды из launcher window. Если launcher ещё не имеет
      // commands API (race с preload exposure), коротко поллим до 3s.
      // Это всё ещё минимальный warm-up vs нормальные 2.5s — оставляем
      // условиям test'а максимально близкими к real-world cold launch.
      await invokeEdenOpenFromLauncher(app);

      const extensionWindow = await app.waitForEvent("window", { timeout: 10_000 });
      await extensionWindow.waitForLoadState("domcontentloaded");
      await waitForExtensionArkBridge(extensionWindow);

      // Убеждаемся, что «ark bridge not ready» нигде не прилетело за это
      // время — ни в console, ни в page errors. (Сами по себе IPC reject'ы
      // в renderer'е логируются как unhandled rejections, поэтому смотрим
      // оба канала.)
      const allErrors = [...consoleErrors, ...pageErrors];
      const bridgeErrors = allErrors.filter((e) => e.includes("ark bridge not ready"));
      if (bridgeErrors.length > 0) {
        throw new Error(`Получены "ark bridge not ready" ошибки: ${JSON.stringify(bridgeErrors)}`);
      }
    } finally {
      await app.close();
    }
  });
});
