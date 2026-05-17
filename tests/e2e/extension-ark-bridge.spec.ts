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
// Этот тест открывает Horologion СРАЗУ через command bus (без 2s warm-up),
// захватывает console errors из extension renderer'а и проверяет, что
// никаких «ark bridge not ready» в первые секунды нет, а статус-индикатор
// в DOM показывает «ARK подключен».

import { test, expect, type ConsoleMessage, type Page } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

test.describe("extension ark bridge", () => {
  test("Horologion: первый probe не fails — bridge ready-gate работает", async () => {
    const app = await launchKepler({ slug: "extension-ark-bridge" });
    try {
      // Минимальный wait — только дождаться, что launcher window вообще
      // загрузился и preload exposed window.kepler. Никаких 2s sleep'ов:
      // мы специально хотим race с initArkClient.
      const launcherPage = await app.firstWindow();
      await launcherPage.waitForLoadState("domcontentloaded");

      // Собираем все будущие extension page errors. Setup ДО открытия —
      // чтобы не упустить ранние логи.
      const consoleErrors: string[] = [];
      const pageErrors: string[] = [];
      app.on("window", (page: Page) => {
        page.on("console", (msg: ConsoleMessage) => {
          if (msg.type() === "error") {
            consoleErrors.push(msg.text());
          }
        });
        page.on("pageerror", (err: Error) => {
          pageErrors.push(err.message);
        });
      });

      // Триггерим открытие Horologion как можно раньше — через invoke
      // open-команды из launcher window. Если launcher ещё не имеет
      // commands API (race с preload exposure), коротко поллим до 3s.
      // Это всё ещё минимальный warm-up vs нормальные 2.5s — оставляем
      // условиям test'а максимально близкими к real-world cold launch.
      const triggered = await app.evaluate(async ({ BrowserWindow }) => {
        const wins = BrowserWindow.getAllWindows();
        const launcher = wins[0];
        if (!launcher) return "no-launcher";
        const start = Date.now();
        while (Date.now() - start < 3000) {
          const ok = await launcher.webContents.executeJavaScript(`
            (async () => {
              if (typeof window.kepler?.commands?.invoke !== "function") {
                return "no-api";
              }
              try {
                await window.kepler.commands.invoke("horologion:open");
                return "ok";
              } catch (e) {
                return "throw:" + (e && e.message ? e.message : String(e));
              }
            })()
          `) as string;
          if (ok === "ok") return ok;
          await new Promise((r) => setTimeout(r, 100));
        }
        return "timeout";
      });
      expect(triggered, `commands.invoke status: ${triggered}`).toBe("ok");

      const horoWindow = await app.waitForEvent("window", { timeout: 10_000 });
      await horoWindow.waitForLoadState("domcontentloaded");

      // Ждём пока индикатор станет «connected». В App.vue класс на dot
      // выставляется по arkStatus: `dot dot--connected | --connecting | --error`.
      // Polling: 5s overall, шаг 100ms.
      const dot = horoWindow.locator(".dot");
      await expect(dot).toHaveClass(/dot--connected/, { timeout: 5_000 });

      // Дополнительная проверка: title-атрибут кнопки статуса должен быть
      // «ARK подключен» (см. arkStatusMessage в App.vue).
      const btn = horoWindow.locator(".ark-status-btn");
      await expect(btn).toHaveAttribute("title", "ARK подключен");

      // Убеждаемся, что «ark bridge not ready» нигде не прилетело за это
      // время — ни в console, ни в page errors. (Сами по себе IPC reject'ы
      // в renderer'е логируются как unhandled rejections, поэтому смотрим
      // оба канала.)
      const allErrors = [...consoleErrors, ...pageErrors];
      const bridgeErrors = allErrors.filter((e) =>
        e.includes("ark bridge not ready"),
      );
      if (bridgeErrors.length > 0) {
        throw new Error(
          `Получены "ark bridge not ready" ошибки: ${JSON.stringify(bridgeErrors)}`,
        );
      }
    } finally {
      await app.close();
    }
  });
});
