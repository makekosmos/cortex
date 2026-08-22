import fs from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const runRoot = path.join(root, ".e2e", "focus", `${process.pid}-${Date.now()}`);
const dataDir = path.join(runRoot, "data");
const lockPath = path.join(dataDir, "engine.lock.json");
const engineBinary = path.resolve(root, "..", "..", "target", "debug", "kepler-backend.exe");
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
function lock() {
  try {
    const value = JSON.parse(fs.readFileSync(lockPath, "utf8")) as {
      pid: number;
      http_port: number;
      auth_token: string;
    };
    return value.pid > 0 && value.http_port > 0 ? value : null;
  } catch {
    return null;
  }
}
async function engineRpc(
  lockValue: { http_port: number; auth_token: string },
  operation: string,
  input: Record<string, unknown> = {},
) {
  const response = await fetch(`http://127.0.0.1:${lockValue.http_port}/v1/rpc`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lockValue.auth_token}`,
      "Content-Type": "application/json",
      "X-Kosmos-Api-Version": "1.0.0",
      "X-Kosmos-Client-Class": "focus-fixture",
      "X-Kosmos-Client-Version": "test",
      "X-Kosmos-Client-Pid": String(process.pid),
    },
    body: JSON.stringify({ operation, _req_id: randomUUID(), ...input }),
  });
  return (await response.json()) as { ok: boolean; data?: unknown };
}
async function waitLock() {
  for (let i = 0; i < 150; i++) {
    const value = lock();
    if (value) return value;
    await wait(200);
  }
  throw new Error("Engine lock timeout");
}
async function launch(
  slot: string,
  serviceState: "absent" | "installed",
  action = "",
): Promise<ElectronApplication> {
  const userData = path.join(runRoot, slot, "userdata");
  fs.mkdirSync(userData, { recursive: true });
  return electron.launch({
    executablePath: electronBinary,
    cwd: root,
    args: [`--user-data-dir=${userData}`, path.join(root, "dist-electron", "main.js")],
    env: {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
      KOSMOS_TEST_FOCUS_SERVICE: serviceState,
      KOSMOS_TEST_FOCUS_SERVICE_ACTION: action,
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      NODE_ENV: "test",
    },
  });
}
async function launchMissing(slot: string): Promise<ElectronApplication> {
  const userData = path.join(runRoot, slot, "userdata");
  fs.mkdirSync(userData, { recursive: true });
  return electron.launch({
    executablePath: electronBinary,
    cwd: root,
    args: [`--user-data-dir=${userData}`, path.join(root, "dist-electron", "missing-main.js")],
    env: { ...process.env, KOSMOS_DATA_DIR: dataDir, KOSMOS_HEADLESS: "1", KOSMOS_TEST_MODE: "1" },
    timeout: 30_000,
  });
}

test("Manager owns Focus settings and stays headless", async () => {
  fs.rmSync(runRoot, { recursive: true, force: true });
  fs.mkdirSync(dataDir, { recursive: true });
  const legacySettingsPath = path.join(dataDir, "kepler-shell-settings.json");
  const legacySettings = JSON.stringify({
    focusServiceAutoInstallDeclined: true,
    retained: "fixture",
  });
  fs.writeFileSync(legacySettingsPath, legacySettings);
  let engine: ChildProcess | undefined;
  let manager: ElectronApplication | undefined;
  let reopened: ElectronApplication | undefined;
  try {
    engine = spawn(engineBinary, [], {
      cwd: root,
      stdio: "ignore",
      env: {
        ...process.env,
        KOSMOS_DATA_DIR: dataDir,
        KOSMOS_TEST_MODE: "1",
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
        KEPLER_SKIP_SYNC: "1",
      },
    });
    const engineLock = await waitLock();
    await engineRpc(engineLock, "focus.upsert_blocklist", {
      id: "fixture-domains",
      name: "Fixture Domains",
      domains: ["example.com"],
      kind: "domains",
      icon: "🌐",
    });
    await engineRpc(engineLock, "focus.upsert_blocklist", {
      id: "fixture-raw",
      name: "Fixture Raw",
      domains: ["@fixture-domains", "news.example"],
      kind: "raw",
      icon: "🧩",
    });
    await engineRpc(engineLock, "focus.set_active_state", {
      active: true,
      blocklist_id: "fixture-raw",
    });
    await expect(launchMissing("failed")).rejects.toThrow();
    expect(fs.readFileSync(legacySettingsPath, "utf8")).toBe(legacySettings);
    manager = await launch("first", "absent", "ok");
    const page = await manager.firstWindow();
    await page.getByRole("button", { name: "Фокус" }).click();
    await expect(page.getByRole("heading", { name: "Системная служба" })).toBeVisible();
    await expect(page.getByText("Не установлена")).toBeVisible();
    const malformed = await page.evaluate(() =>
      window.kosmosManager.upsertFocusBlocklist({ name: "bad", domains: [] } as never),
    );
    expect(malformed).toMatchObject({ ok: false, code: "validation" });
    const activeState = await page.evaluate(() => window.kosmosManager.getFocusActiveState());
    expect(activeState).toMatchObject({
      ok: true,
      data: { active: true, blocklist_id: "fixture-raw" },
    });
    const activeCard = page
      .locator(".focus-grid [role='button']")
      .filter({ hasText: "Fixture Raw" });
    await activeCard.click();
    await expect(page.getByRole("alert")).toContainText("Нельзя изменить");
    await activeCard.getByRole("button", { name: "Удалить" }).click();
    await expect(page.getByRole("alert")).toContainText("Нельзя удалить");
    const inactiveCard = page
      .locator(".focus-grid [role='button']")
      .filter({ hasText: "Fixture Domains" });
    await inactiveCard.click();
    await expect(page.getByRole("heading", { name: "Изменить блок-лист" })).toBeVisible();
    await page.locator("form input").first().fill("Fixture Domains Updated");
    await page.getByRole("button", { name: "Сохранить" }).click();
    await expect(page.getByText("Fixture Domains Updated")).toBeVisible();
    await page
      .locator(".focus-grid [role='button']")
      .filter({ hasText: "Fixture Domains Updated" })
      .getByRole("button", { name: "Удалить" })
      .click();
    await expect(page.getByText("Fixture Domains Updated")).toHaveCount(0);
    const serviceAction = await page.evaluate(() => window.kosmosManager.installFocusService());
    expect(serviceAction).toEqual({ ok: true, data: { ok: true } });
    const created = await page.evaluate(() =>
      window.kosmosManager.upsertFocusBlocklist({
        name: "Тест",
        domains: ["example.com"],
        kind: "domains",
      }),
    );
    expect(created.ok).toBe(true);
    await page.getByRole("button", { name: "Создать" }).click();
    await expect(page.getByRole("heading", { name: "Новый блок-лист" })).toBeVisible();
    const windows = await manager.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows().map((window) => ({
        visible: window.isVisible(),
        focused: window.isFocused(),
      })),
    );
    expect(windows).toEqual([{ visible: false, focused: false }]);
    await manager.close();
    manager = undefined;
    reopened = await launch("reopen", "installed");
    expect(
      await (
        await reopened.firstWindow()
      ).evaluate(() => window.kosmosManager.getFocusServiceStatus()),
    ).toEqual({
      ok: true,
      data: { installed: true, running: true, healthy: true },
    });
    expect(fs.readFileSync(legacySettingsPath, "utf8")).toBe(legacySettings);
    const reopenedPage = await reopened.firstWindow();
    await reopenedPage.getByRole("button", { name: "Фокус" }).click();
    await expect(reopenedPage.getByText("Блок-листы")).toBeVisible();
    const retained = await reopenedPage.evaluate(() => window.kosmosManager.getFocusBlocklists());
    expect(
      retained.ok &&
        retained.data.some(
          (item) =>
            item.name === "Fixture Raw" &&
            item.kind === "raw" &&
            item.domains.includes("@fixture-domains"),
        ),
    ).toBe(true);
    spawnSync(engineBinary, ["--shutdown"], {
      cwd: root,
      env: {
        ...process.env,
        KOSMOS_DATA_DIR: dataDir,
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      },
      stdio: "ignore",
    });
    await wait(500);
    const failedEdit = await reopenedPage.evaluate(() =>
      window.kosmosManager.upsertFocusBlocklist({
        id: "fixture-raw",
        name: "Should Not Persist",
        domains: ["blocked.example"],
        kind: "raw",
      }),
    );
    expect(failedEdit).toMatchObject({ ok: false, code: "engine" });
    const failedDelete = await reopenedPage.evaluate(() =>
      window.kosmosManager.deleteFocusBlocklist({ id: "fixture-raw" }),
    );
    expect(failedDelete).toMatchObject({ ok: false, code: "engine" });
    await reopened.close();
    reopened = undefined;
  } finally {
    await reopened?.close().catch(() => undefined);
    await manager?.close().catch(() => undefined);
    if (engine) {
      spawnSync(engineBinary, ["--shutdown"], {
        cwd: root,
        env: {
          ...process.env,
          KOSMOS_DATA_DIR: dataDir,
          KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
        },
        stdio: "ignore",
      });
      engine.kill();
    }
    await wait(500);
    fs.rmSync(runRoot, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  }
});
