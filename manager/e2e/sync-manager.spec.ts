import fs from "node:fs";
import { createServer, type Server } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";

const managerRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const runRoot = path.resolve(
  managerRoot,
  "..",
  "..",
  "tests",
  ".e2e",
  "sync-manager",
  `${process.pid}-${Date.now()}`,
);
const dataDir = path.join(runRoot, "data");
const fixtureTicket = "fixture-ticket-123";

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

type SyncFixture = {
  unavailable: boolean;
  ticketFailure: boolean;
  connectFailure: boolean;
  disconnectFailure: boolean;
  concurrentSnapshots: number;
  maxConcurrentSnapshots: number;
  connectCalls: number;
  connected: boolean;
  disconnected: boolean;
  close(): Promise<void>;
};

async function startSyncFixture(): Promise<SyncFixture> {
  const fixture = {
    unavailable: false,
    ticketFailure: false,
    connectFailure: false,
    disconnectFailure: false,
    concurrentSnapshots: 0,
    maxConcurrentSnapshots: 0,
    connectCalls: 0,
    connected: false,
    disconnected: false,
  };
  const server: Server = createServer(async (request, response) => {
    const send = (body: unknown) => {
      response.writeHead(200, { "Content-Type": "application/json" });
      response.end(JSON.stringify(body));
    };
    if (request.url === "/v1/health")
      return send({ ok: true, status: "ready", api_version: "1.0.0" });
    if (request.url !== "/v1/rpc" || request.method !== "POST") return send({ ok: false });
    const chunks: Buffer[] = [];
    for await (const chunk of request) chunks.push(Buffer.from(chunk));
    const body = JSON.parse(Buffer.concat(chunks).toString("utf8")) as Record<string, unknown>;
    if (body.operation === "get_sync_snapshot") {
      fixture.concurrentSnapshots += 1;
      fixture.maxConcurrentSnapshots = Math.max(
        fixture.maxConcurrentSnapshots,
        fixture.concurrentSnapshots,
      );
      await delay(25);
      fixture.concurrentSnapshots -= 1;
      if (fixture.unavailable) return send({ ok: false, error: "fixture unavailable" });
      return send({
        ok: true,
        data: {
          running: true,
          transport: "iroh",
          pairing_available: true,
          own_pairing_code_available: true,
          local_device: { device_id: "fixture-manager", device_name: "Тестовый Manager" },
          peers: [
            !fixture.disconnected && {
              device_id: "fixture-online",
              device_name: "Телефон",
              status: "online",
              last_seen: "2026-08-01T10:20:30Z",
            },
            {
              device_id: "fixture-offline",
              device_name: "Планшет",
              status: "offline",
              last_seen: "2026-08-01T09:20:30Z",
            },
            fixture.connected && {
              device_id: "fixture-connected",
              device_name: "Подключённое устройство",
              status: "online",
              last_seen: "2026-08-01T10:21:30Z",
            },
          ].filter(Boolean),
        },
      });
    }
    if (body.operation === "get_own_iroh_ticket")
      return fixture.ticketFailure
        ? send({ ok: false, error: "secret-ticket-failure" })
        : send({ ok: true, data: ` ${fixtureTicket} ` });
    if (body.operation === "connect_with_pairing_code") {
      fixture.connectCalls += 1;
      if (fixture.connectFailure) return send({ ok: false, error: "secret-connect-failure" });
      fixture.connected = body.pairing_code === "valid-code-123";
      return send({ ok: true, data: true });
    }
    if (body.operation === "disconnect_peer") {
      if (fixture.disconnectFailure) return send({ ok: false, error: "secret-disconnect-failure" });
      fixture.disconnected = body.device_id === "fixture-online";
      return send({ ok: true, data: true });
    }
    return send({ ok: true, data: {} });
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Sync fixture did not bind a port");
  fs.mkdirSync(dataDir, { recursive: true });
  fs.writeFileSync(
    path.join(dataDir, "engine.lock.json"),
    JSON.stringify({
      format_version: 1,
      api_version: { major: 1, minor: 0, patch: 0 },
      pid: process.pid,
      http_port: address.port,
      ws_port: 4318,
      auth_token: "a".repeat(64),
      started_at: "2026-08-01T00:00:00Z",
      correlation_id: "11111111-1111-4111-8111-111111111111",
    }),
  );
  return Object.assign(fixture, {
    close: () =>
      new Promise<void>((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      ),
  });
}

async function launchManager(slot: string): Promise<ElectronApplication> {
  const userDataDir = path.join(runRoot, slot, "userdata");
  fs.mkdirSync(userDataDir, { recursive: true });
  return electron.launch({
    executablePath: electronBinary,
    cwd: managerRoot,
    args: [`--user-data-dir=${userDataDir}`, path.join(managerRoot, "dist-electron", "main.js")],
    env: {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      NODE_ENV: "test",
    },
  });
}

test("Manager Sync uses the isolated deterministic fixture", async () => {
  fs.rmSync(runRoot, { recursive: true, force: true });
  let fixture: SyncFixture | undefined;
  let manager: ElectronApplication | undefined;
  let unavailableManager: ElectronApplication | undefined;
  try {
    fixture = await startSyncFixture();
    manager = await launchManager("ready");
    const page = await manager.firstWindow();
    await page.getByRole("button", { name: "Синхронизация" }).click();
    await expect(page.getByText("Это устройство: Тестовый Manager")).toBeVisible();
    await expect(page.getByText("Транспорт: Iroh")).toBeVisible();
    await expect(page.getByText(/Телефон.*Онлайн.*Был в сети/)).toBeVisible();
    await expect(page.getByText(/Планшет.*Оффлайн/)).toBeVisible();

    await page.getByRole("button", { name: "Показать код" }).click();
    await expect(page.getByText(fixtureTicket)).toBeVisible();
    fixture.ticketFailure = true;
    await page.locator(".card").nth(1).getByRole("button").first().click();
    await expect(page.getByRole("alert")).toHaveText("Engine отклонил операцию.");
    await expect(page.getByRole("alert")).not.toContainText("secret-ticket-failure");
    await expect(page.getByText(fixtureTicket)).toHaveCount(0);
    fixture.ticketFailure = false;
    await page.locator(".card").nth(1).getByRole("button").first().click();
    await expect(page.getByText(fixtureTicket)).toBeVisible();
    await page.getByRole("button", { name: "Копировать" }).click();
    await expect(page.getByRole("button", { name: "Скопировано" })).toBeVisible();

    const pairingInput = page.getByLabel("Код сопряжения");
    await pairingInput.fill("1234567");
    await page.getByRole("button", { name: "Подключить" }).click();
    await expect(page.getByRole("alert")).toHaveText(
      "Введите корректный код сопряжения (не менее 8 символов).",
    );
    expect(fixture.connectCalls).toBe(0);
    fixture.connectFailure = true;
    await pairingInput.fill("  valid-code-123  ");
    await page.locator(".card").nth(1).getByRole("button").last().click();
    await expect(page.getByRole("alert")).toHaveText("Engine отклонил операцию.");
    await expect(page.getByRole("alert")).not.toContainText("secret-connect-failure");
    await expect(pairingInput).toHaveValue("  valid-code-123  ");
    fixture.connectFailure = false;

    await pairingInput.fill("  valid-code-123  ");
    await page.getByRole("button", { name: "Подключить" }).click();
    await expect(page.getByText(/Подключённое устройство.*Онлайн/)).toBeVisible();
    expect(fixture.connectCalls).toBe(2);

    fixture.disconnectFailure = true;
    page.once("dialog", (dialog) => void dialog.accept());
    await page.locator(".row").first().getByRole("button").click();
    await expect(page.getByRole("alert")).toHaveText("Engine отклонил операцию.");
    await expect(page.getByRole("alert")).not.toContainText("secret-disconnect-failure");
    await expect(page.getByText(/Телефон.*Онлайн/)).toBeVisible();
    fixture.disconnectFailure = false;
    page.once("dialog", (dialog) => void dialog.accept());
    await page.locator(".row").first().getByRole("button").click();
    await expect(page.getByText(/Телефон.*Онлайн/)).toHaveCount(0);

    await page.getByRole("button", { name: "Повторить запрос" }).evaluate((button) => {
      button.click();
      button.click();
      button.click();
    });
    await delay(100);
    expect(fixture.maxConcurrentSnapshots).toBe(1);
    const windows = await manager.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows().map((win) => win.isVisible()),
    );
    expect(windows).toEqual([false]);

    await manager.close();
    manager = undefined;
    fixture.unavailable = true;
    unavailableManager = await launchManager("unavailable");
    const unavailablePage = await unavailableManager.firstWindow();
    await unavailablePage.getByRole("button", { name: "Синхронизация" }).click();
    await expect(unavailablePage.getByText("Недоступна")).toBeVisible();
    await expect(unavailablePage.getByRole("button", { name: "Показать код" })).toBeDisabled();
    await expect(unavailablePage.getByRole("button", { name: "Подключить" })).toBeDisabled();
  } finally {
    await unavailableManager?.close().catch(() => undefined);
    await manager?.close().catch(() => undefined);
    await fixture?.close().catch(() => undefined);
    fs.rmSync(runRoot, { recursive: true, force: true });
  }
});
