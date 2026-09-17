import fs from "node:fs";
import path from "node:path";
import { test, expect } from "@playwright/test";
import type { ElectronApplication } from "playwright";
import {
  delay,
  fixtureAuthToken,
  fixtureTicket,
  startSyncFixture,
  type SyncFixture,
} from "./sync-fixture";
import {
  cleanupManifest,
  closeHost,
  launchManager,
  managerE2eRoot,
  managerEnvironment,
  managerMain,
  recordCleanup,
  waitForPidGone,
} from "./manager-runtime";

test("Manager Sync uses the isolated deterministic fixture", async () => {
  test.skip(!fs.existsSync(managerMain), `build Manager first: ${managerMain}`);

  const runRoot = managerE2eRoot("sync-manager");
  const dataDir = path.join(runRoot, "data");
  const managerEnv = managerEnvironment(runRoot, dataDir);
  const pids = new Set<number>();
  let fixture: SyncFixture | undefined;
  let manager: ElectronApplication | undefined;
  let unavailableManager: ElectronApplication | undefined;
  try {
    fixture = await startSyncFixture(dataDir);
    manager = await launchManager(runRoot, "ready", managerEnv);
    pids.add(manager.process().pid);
    const page = await manager.firstWindow();
    await page.getByRole("button", { name: "Синхронизация" }).click();
    await expect(page.getByText("Тестовый Manager")).toBeVisible();
    await expect(page.getByText("Транспорт: Iroh")).toBeVisible();
    const peers = page.locator(".settings-row");
    await expect(peers.filter({ hasText: "Телефон" })).toContainText(/Онлайн.*Был в сети/);
    await expect(peers.filter({ hasText: "Планшет" })).toContainText("Оффлайн");

    const openPairing = page.getByRole("button", { name: "Открыть сопряжение" });
    const pairingDialog = page.getByRole("dialog");
    const closePairing = pairingDialog.getByRole("button", { name: "Закрыть" });
    await openPairing.click();
    await expect(pairingDialog).toBeVisible();
    await expect(page.getByText(fixtureTicket)).toBeVisible();
    fixture.ticketFailure = true;
    await closePairing.click();
    await expect(pairingDialog).toHaveCount(0);
    await openPairing.click();
    await expect(page.getByText("Код недоступен.")).toBeVisible();
    await expect(page.getByText("Менеджер недоступен")).toBeVisible();
    await expect(page.getByText(fixtureTicket)).toHaveCount(0);
    fixture.ticketFailure = false;
    await closePairing.click();
    await expect(pairingDialog).toHaveCount(0);
    await openPairing.click();
    await expect(page.getByText(fixtureTicket)).toBeVisible();
    await pairingDialog.getByRole("button", { name: "Копировать" }).click();
    await expect(pairingDialog.getByRole("button", { name: "Скопировано" })).toBeVisible();

    const pairingInput = page.getByPlaceholder("Код сопряжения");
    const connectButton = page.getByRole("button", {
      name: "Подключить устройство",
    });
    await pairingInput.fill("1234567");
    await connectButton.click();
    await expect(
      page.getByText("Введите корректный код сопряжения (не менее 8 символов)."),
    ).toBeVisible();
    expect(fixture.connectCalls).toBe(0);
    fixture.connectFailure = true;
    await pairingInput.fill("  valid-code-123  ");
    await connectButton.click();
    await expect(page.getByText("Менеджер недоступен")).toBeVisible();
    await expect(pairingInput).toHaveValue("  valid-code-123  ");
    fixture.connectFailure = false;

    await pairingInput.fill("  valid-code-123  ");
    await connectButton.click();
    await expect(pairingDialog).toHaveCount(0);
    await expect(page.getByText("Подключённое устройство")).toBeVisible();
    await expect(peers.filter({ hasText: "Подключённое устройство" })).toContainText("Онлайн");
    expect(fixture.connectCalls).toBe(2);

    const onlinePeer = peers.filter({ hasText: "Телефон" });
    fixture.disconnectFailure = true;
    page.once("dialog", (dialog) => void dialog.accept());
    await onlinePeer.getByRole("button", { name: "Отключить" }).click();
    await expect(page.getByText("Менеджер недоступен")).toBeVisible();
    await expect(onlinePeer).toContainText("Онлайн");
    fixture.disconnectFailure = false;
    page.once("dialog", (dialog) => void dialog.accept());
    await onlinePeer.getByRole("button", { name: "Отключить" }).click();
    await expect(peers.filter({ hasText: "Телефон" })).toHaveCount(0);

    await page.getByRole("button", { name: "Обновить" }).evaluate((element) => {
      // SAFETY: getByRole("button") resolves a native <button>, always an HTMLButtonElement.
      const button = element as HTMLButtonElement;
      for (let click = 0; click < 3; click += 1) button.click();
    });
    await delay(100);
    expect(fixture.maxConcurrentSnapshots).toBe(1);
    // The Engine lock auth_token is a real secret and must never render.
    expect(await page.locator("body").innerText()).not.toContain(fixtureAuthToken);
    const windows = await manager.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows().map((win) => win.isVisible()),
    );
    expect(windows).toEqual([false]);

    await closeHost(manager, pids);
    manager = undefined;
    fixture.unavailable = true;
    unavailableManager = await launchManager(runRoot, "unavailable", managerEnv);
    pids.add(unavailableManager.process().pid);
    const unavailablePage = await unavailableManager.firstWindow();
    await unavailablePage.getByRole("button", { name: "Синхронизация" }).click();
    await expect(unavailablePage.getByRole("button", { name: "Недоступна" })).toBeVisible();
    await unavailablePage.getByRole("button", { name: "Открыть сопряжение" }).click();
    await expect(unavailablePage.getByText("Код недоступен.")).toBeVisible();
  } finally {
    const cleanupErrors: unknown[] = [];
    const attempt = async (action: () => Promise<void>) => {
      try {
        await action();
      } catch (error) {
        cleanupErrors.push(error);
      }
    };
    await attempt(() => closeHost(unavailableManager, pids));
    await attempt(() => closeHost(manager, pids));
    await attempt(async () => {
      await fixture?.close();
    });
    for (const pid of pids) {
      await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    }
    try {
      recordCleanup(cleanupManifest(), runRoot, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "sync manager E2E cleanup failed").toHaveLength(0);
  }
});
