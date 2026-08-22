import { test, expect } from "@playwright/test";
import {
  freshDataDir,
  launchKeplerWithDataDir,
  shutdownKeplerEngine,
} from "../../../../../tests/e2e/helpers/launch";
import { waitForBackendReady } from "../../../../../tests/e2e/helpers/wait";

async function visibleWindows(app: Awaited<ReturnType<typeof launchKeplerWithDataDir>>) {
  return app.evaluate(({ BrowserWindow }) =>
    BrowserWindow.getAllWindows()
      .filter((win) => !win.isDestroyed() && win.isVisible())
      .map((win) => ({ id: win.id, title: win.getTitle() })),
  );
}

test("headless repeat-open does not show existing settings/dashboard windows", async () => {
  const dataDir = freshDataDir("headless-repeat-open");

  const app = await launchKeplerWithDataDir(dataDir);
  try {
    const launcher = await app.firstWindow();
    await launcher.waitForLoadState("domcontentloaded");
    await waitForBackendReady(launcher);

    await launcher.evaluate(async () => {
      await window.kepler.commands.invoke("settings:open");
    });

    await expect
      .poll(async () => {
        return app.evaluate(
          ({ BrowserWindow }) =>
            BrowserWindow.getAllWindows().filter(
              (win) => !win.isDestroyed() && win.getTitle() === "Kosmos — Настройки",
            ).length,
        );
      })
      .toBe(1);

    await launcher.evaluate(async () => {
      await window.kepler.settings.open();
    });

    await expect
      .poll(async () => {
        return app.evaluate(
          ({ BrowserWindow }) =>
            BrowserWindow.getAllWindows().filter(
              (win) => !win.isDestroyed() && win.getTitle() === "Kosmos — Настройки",
            ).length,
        );
      })
      .toBe(1);

    await launcher.evaluate(async () => {
      await window.kepler.commands.invoke("dashboard:open");
      await window.kepler.commands.invoke("dashboard:open");
    });

    await app.evaluate(({ app: electronApp }) => {
      electronApp.emit("second-instance", {} as never, ["Kosmos.exe", "--autostart"]);
      electronApp.emit("second-instance", {} as never, ["Kosmos.exe", "--autostart"]);
    });

    await expect.poll(async () => visibleWindows(app), { timeout: 5_000 }).toEqual([]);
  } finally {
    shutdownKeplerEngine(dataDir);
    await app.close();
  }
});
