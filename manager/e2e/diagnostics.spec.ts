import fs from "node:fs";
import path from "node:path";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";

const managerRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const engineBinary = path.resolve(managerRoot, "..", "..", "target", "debug", "kepler-backend.exe");
const runRoot = path.join(managerRoot, ".e2e", "diagnostics", `${process.pid}-${Date.now()}`);
const dataDir = process.env.KOSMOS_DATA_DIR || path.join(runRoot, "data");
const lockPath = path.join(dataDir, "engine.lock.json");

function wait(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
async function waitForLock() {
  for (let i = 0; i < 150; i += 1) {
    try {
      const lock = JSON.parse(fs.readFileSync(lockPath, "utf8")) as {
        pid: number;
        http_port: number;
      };
      if (lock.pid > 0 && lock.http_port > 0) return lock;
    } catch {
      /* wait */
    }
    await wait(200);
  }
  throw new Error("Engine lock was not created");
}

test("Manager diagnostics stays metadata-only and headless-safe", async () => {
  fs.rmSync(runRoot, { recursive: true, force: true });
  fs.mkdirSync(path.join(dataDir, "crashes"), { recursive: true });
  fs.writeFileSync(
    path.join(dataDir, "crashes", "fixture.log"),
    "raw crash payload must stay hidden",
  );
  fs.writeFileSync(path.join(dataDir, "crashes", "ignore.txt"), "ignore");
  const engine = spawn(engineBinary, [], {
    cwd: managerRoot,
    stdio: "ignore",
    env: {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KEPLER_SKIP_SYNC: "1",
      RUST_LOG: "error",
    },
  });
  let manager: ElectronApplication | undefined;
  try {
    await waitForLock();
    manager = await electron.launch({
      executablePath: electronBinary,
      cwd: managerRoot,
      args: [
        `--user-data-dir=${path.join(runRoot, "userdata")}`,
        path.join(managerRoot, "dist-electron", "main.js"),
      ],
      env: {
        ...process.env,
        KOSMOS_DATA_DIR: dataDir,
        KOSMOS_HEADLESS: "1",
        KOSMOS_TEST_MODE: "1",
        KOSMOS_CAPTURE_OFFSCREEN: "1",
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      },
    });
    const page = await manager.firstWindow();
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    await page.getByRole("button", { name: "Диагностика" }).click();
    await expect(page.getByRole("heading", { name: "Пакет поддержки" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Рабочие процессы" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Последние сообщения" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Сохранить пакет поддержки" })).toBeVisible();
    await expect(page.getByText("fixture.log")).toBeVisible();
    await expect(page.getByText("raw crash payload must stay hidden")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Открыть логи" })).toBeVisible();
    const snapshot = await page.evaluate(() => window.kosmosManager.getDiagnosticsSnapshot());
    expect(snapshot.ok).toBe(true);
    const tail = await page.evaluate(() =>
      window.kosmosManager.getDiagnosticLogTail({ lines: 100 }),
    );
    expect(tail.ok).toBe(true);
    const support = await page.evaluate(() => window.kosmosManager.saveSupportBundle());
    expect(support).toEqual({
      ok: true,
      data: { saved: false, cancelled: true },
    });
    const openedLogs = await page.evaluate(() => window.kosmosManager.openLogsFolder());
    expect(openedLogs).toEqual({ ok: true, data: { opened: false } });
    const openedCrashes = await page.evaluate(() => window.kosmosManager.openCrashReportsFolder());
    expect(openedCrashes).toEqual({ ok: true, data: { opened: false } });
    const body = await page.locator("body").innerText();
    expect(body).not.toContain(dataDir);
    expect(body).not.toMatch(/[A-Z]:\\[^\n]+/i);
    await page.getByRole("button", { name: "Очистить" }).click();
    await expect(page.getByText("Отчётов нет.")).toBeVisible();
    await page.reload();
    await expect(page.locator(".eyebrow")).toHaveCount(0);
    await expect(page.locator(".page-header").getByRole("button")).toHaveCount(0);
    await expect(page.getByText("raw crash payload must stay hidden")).toHaveCount(0);
    expect(await page.locator("body").innerText()).not.toContain(dataDir);
    const evidenceDir = process.env.KOSMOS_VISUAL_EVIDENCE_DIR;
    for (const section of [
      "Данные",
      "Синхронизация",
      "Маркетплейс",
      "Движок",
      "Настройки",
      "Диагностика",
      "Подключения",
      "О приложении",
      "Обновления",
    ]) {
      await page.getByRole("button", { name: section, exact: true }).click();
      await expect(
        page.getByRole("heading", { name: section, exact: true, level: 1 }),
      ).toBeVisible();
      if (section === "О приложении") {
        await expect(page.getByText("Загрузка…", { exact: true })).toHaveCount(0);
      }
      if (section === "Подключения") {
        await expect(page.locator(".connection-card")).toHaveCount(4);
        await expect(page.locator(".connection-card").first()).toHaveAttribute(
          "aria-label",
          /Подключено|Не подключено/,
        );
        await page.locator(".connection-card").first().click();
        await expect(page.getByRole("dialog")).toBeVisible();
        await expect(page.getByRole("dialog").getByText("Синхронизация")).toBeVisible();
        await expect(
          page.getByRole("dialog").getByRole("button", { name: "Закрыть" }),
        ).toBeVisible();
        await page.getByRole("dialog").getByRole("button", { name: "Закрыть" }).click();
        await expect(page.getByRole("dialog")).toHaveCount(0);
      }
      if (section === "Обновления") {
        await expect(page.getByRole("button", { name: "Проверить обновления" })).toBeVisible();
        await expect(page.getByRole("button", { name: "Обновить всё" })).toBeVisible();
        await expect(
          page.getByText("Проверка Desktop и приложений через их штатные каналы."),
        ).toHaveCount(0);
        for (const title of [
          "Kosmos Desktop",
          "Kosmos Shell",
          "Eden",
          "Delphi",
          "Cosmos Graph",
          "Dictation",
        ])
          await expect(page.getByText(title, { exact: true })).toBeVisible();
        for (const id of [
          "com.kosmos.shell",
          "com.kosmos.eden",
          "com.kosmos.delphi",
          "com.kosmos.graph",
          "com.kosmos.dictation",
        ])
          await expect(page.getByText(id, { exact: true })).toHaveCount(0);
      }
      if (evidenceDir) {
        await page.waitForTimeout(250);
        fs.mkdirSync(evidenceDir, { recursive: true });
        const png = await manager.evaluate(async ({ BrowserWindow }) => {
          const contents = BrowserWindow.getAllWindows()[0].webContents;
          contents.invalidate();
          return (await contents.capturePage()).toPNG().toString("base64");
        });
        fs.writeFileSync(path.join(evidenceDir, `${section.toLowerCase()}.png`), png, "base64");
      }
    }
    const version = await page.evaluate(() => window.kosmosManager.getAppVersion());
    expect(version.ok).toBe(true);
    if (version.ok) await expect(page.getByText(version.data, { exact: true })).toBeVisible();
    expect(errors).toEqual([]);
    const windows = await manager.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows().map((window) => window.isVisible()),
    );
    expect(windows).toEqual([false]);
  } finally {
    await manager?.close().catch(() => undefined);
    spawnSync(engineBinary, ["--shutdown"], {
      cwd: managerRoot,
      env: {
        ...process.env,
        KOSMOS_DATA_DIR: dataDir,
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      },
      stdio: "ignore",
    });
    await wait(500);
    if (engine.exitCode === null) engine.kill();
    if (!process.env.KOSMOS_DATA_DIR) fs.rmSync(runRoot, { recursive: true, force: true });
  }
});
