import fs from "node:fs";
import path from "node:path";
import { test, expect } from "@playwright/test";
import type { ElectronApplication } from "playwright";
import {
  cleanupManifest,
  closeHost,
  engineBinaries,
  launchManager,
  managerE2eRoot,
  managerEnvironment,
  managerMain,
  recordCleanup,
  startEngine,
  terminate,
  waitForPidGone,
} from "./manager-runtime";

test("Manager diagnostics stays metadata-only and headless-safe", async () => {
  test.skip(!fs.existsSync(managerMain), `build Manager first: ${managerMain}`);

  const runRoot = managerE2eRoot("diagnostics");
  const dataDir = process.env.KOSMOS_DATA_DIR || path.join(runRoot, "data");
  const binaries = engineBinaries();
  const managerEnv = managerEnvironment(runRoot, dataDir, {
    KOSMOS_CAPTURE_OFFSCREEN: "1",
  });
  const pids = new Set<number>();
  fs.mkdirSync(path.join(dataDir, "crashes"), { recursive: true });
  fs.writeFileSync(
    path.join(dataDir, "crashes", "fixture.log"),
    "raw crash payload must stay hidden",
  );
  fs.writeFileSync(path.join(dataDir, "crashes", "ignore.txt"), "ignore");
  let engine: Awaited<ReturnType<typeof startEngine>> | undefined;
  let manager: ElectronApplication | undefined;
  try {
    engine = await startEngine(binaries.engine, binaries.ark, dataDir, {
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
      KEPLER_SKIP_SYNC: "1",
      RUST_LOG: "error",
    });
    if (engine.child.pid) pids.add(engine.child.pid);
    manager = await launchManager(runRoot, "manager", managerEnv);
    pids.add(manager.process().pid);
    const page = await manager.firstWindow();
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    await expect(page.locator("[aria-label='Разделы менеджера']")).toBeVisible();

    // Diagnostics surface is IPC-level: crash reports expose names/sizes only,
    // never file contents or filesystem paths.
    const reports = await page.evaluate(() => window.kosmosManager.listCrashReports());
    expect(reports).toMatchObject({
      ok: true,
      data: [{ name: "fixture.log" }],
    });
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
    const cleared = await page.evaluate(() => window.kosmosManager.clearCrashReports());
    expect(cleared).toMatchObject({ ok: true, data: { removed: 1 } });
    const afterClear = await page.evaluate(() => window.kosmosManager.listCrashReports());
    expect(afterClear).toEqual({ ok: true, data: [] });

    const evidenceDir = process.env.KOSMOS_VISUAL_EVIDENCE_DIR;
    const sections: Array<{
      nav: string;
      check: () => Promise<void>;
    }> = [
      {
        nav: "Данные",
        check: async () => {
          await expect(page.getByText("Управляемое хранилище")).toBeVisible();
        },
      },
      {
        nav: "Синхронизация",
        check: async () => {
          await expect(page.getByText("Состояние синхронизации").first()).toBeVisible();
        },
      },
      {
        nav: "Движок",
        check: async () => {
          await expect(page.getByText("Тёплый рабочий стол")).toBeVisible();
          await expect(page.getByText("Учёт активности")).toBeVisible();
        },
      },
      {
        nav: "Интеграции",
        check: async () => {
          const cards = page.locator(".connection-card");
          await expect(cards.first()).toBeVisible();
          expect(await cards.count()).toBeGreaterThan(0);
          await expect(cards.first()).toHaveAttribute(
            "aria-label",
            /Подключено|Не подключено|Не установлена/,
          );
          await cards.first().click();
          await expect(page.getByRole("dialog")).toBeVisible();
          await page.getByRole("dialog").getByRole("button", { name: "Закрыть" }).click();
          await expect(page.getByRole("dialog")).toHaveCount(0);
        },
      },
      {
        nav: "Ключи",
        check: async () => {
          await expect(page.locator("[aria-label='Ключи']")).toBeVisible();
        },
      },
      {
        nav: "Браузер",
        check: async () => {
          await expect(page.getByText("Хранить данные браузера")).toBeVisible();
        },
      },
      {
        nav: "Маркетплейс",
        check: async () => {
          await expect(page.locator("[aria-label='Маркетплейс']")).toBeVisible();
        },
      },
      {
        nav: "Обновления",
        check: async () => {
          await expect(page.getByText("Kosmos Desktop")).toBeVisible();
          await expect(page.getByText("Приложения Kosmos")).toBeVisible();
          await expect(page.getByRole("button", { name: "Проверить обновления" })).toBeVisible();
        },
      },
      {
        nav: "О приложении",
        check: async () => {
          await expect(page.getByText("Версия Kosmos")).toBeVisible();
        },
      },
      {
        nav: "Настройки",
        check: async () => {
          await expect(page.getByRole("heading", { name: "Резервные копии базы" })).toBeVisible();
        },
      },
    ];
    const sidebar = page.locator("[aria-label='Разделы менеджера']");
    for (const { nav, check } of sections) {
      await sidebar.getByRole("button", { name: nav, exact: true }).click();
      await check();
      if (evidenceDir) {
        await page.waitForTimeout(250);
        fs.mkdirSync(evidenceDir, { recursive: true });
        const png = await manager.evaluate(async ({ BrowserWindow }) => {
          const contents = BrowserWindow.getAllWindows()[0].webContents;
          contents.invalidate();
          return (await contents.capturePage()).toPNG().toString("base64");
        });
        fs.writeFileSync(path.join(evidenceDir, `${nav.toLowerCase()}.png`), png, "base64");
      }
    }

    await page.reload();
    await expect(page.locator("[aria-label='Разделы менеджера']")).toBeVisible();
    const body = await page.locator("body").innerText();
    expect(body).not.toContain("raw crash payload must stay hidden");
    expect(body).not.toContain(dataDir);
    expect(body).not.toMatch(/[A-Z]:\\[^\n]+/i);
    const version = await page.evaluate(() => window.kosmosManager.getAppVersion());
    expect(version.ok).toBe(true);
    // The bundled CSP (`font-src 'self'`) already blocks Manager's own
    // data: fonts before this port; keep the check for unexpected errors.
    const unexpectedErrors = errors.filter(
      (error) => !(error.startsWith("Loading the font") && error.includes("font-src 'self'")),
    );
    expect(unexpectedErrors).toEqual([]);
    const windows = await manager.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows().map((window) => window.isVisible()),
    );
    expect(windows).toEqual([false]);
  } finally {
    const cleanupErrors: unknown[] = [];
    const attempt = async (action: () => Promise<void>) => {
      try {
        await action();
      } catch (error) {
        cleanupErrors.push(error);
      }
    };
    await attempt(() => closeHost(manager, pids));
    await attempt(() => terminate(engine?.child, binaries.engine, dataDir, "Engine"));
    for (const pid of pids) {
      await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    }
    try {
      recordCleanup(cleanupManifest(), runRoot, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "Manager diagnostics E2E cleanup failed").toHaveLength(0);
  }
});
