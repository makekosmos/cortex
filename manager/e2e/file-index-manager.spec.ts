import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
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

const managerRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function engineRpc(
  lock: { http_port: number; auth_token: string },
  operation: string,
  input = {},
) {
  const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lock.auth_token}`,
      "Content-Type": "application/json",
      "X-Kosmos-Api-Version": "1.0.0",
      "X-Kosmos-Client-Class": "file-index-fixture",
      "X-Kosmos-Client-Version": "test",
      "X-Kosmos-Client-Pid": String(process.pid),
    },
    body: JSON.stringify({ operation, ...input }),
  });
  // SAFETY: the fixture endpoint returns the tested RPC envelope.
  return (await response.json()) as { ok: boolean; data?: unknown };
}

test("Manager keeps the File Index API without a sidebar surface", async () => {
  test.skip(!fs.existsSync(managerMain), `build Manager first: ${managerMain}`);

  const runRoot = managerE2eRoot("file-index");
  const dataDir = path.join(runRoot, "data");
  const fixtureRoot = path.join(runRoot, "fixture-root");
  const binaries = engineBinaries();
  const managerEnv = managerEnvironment(runRoot, dataDir);
  const pids = new Set<number>();
  fs.mkdirSync(fixtureRoot, { recursive: true });
  fs.writeFileSync(path.join(fixtureRoot, "fixture.txt"), "fixture");
  let engine: Awaited<ReturnType<typeof startEngine>> | undefined;
  let manager: ElectronApplication | undefined;
  let reopened: ElectronApplication | undefined;
  try {
    engine = await startEngine(binaries.engine, binaries.ark, dataDir, {
      KOSMOS_TEST_MODE: "1",
      KEPLER_SKIP_SYNC: "1",
      RUST_LOG: "error",
    });
    if (engine.child.pid) pids.add(engine.child.pid);
    const lock = engine.lock;
    expect(
      (
        await engineRpc(lock, "file_index.settings_set", {
          enabled: true,
          include_hidden: false,
        })
      ).ok,
    ).toBe(true);
    expect((await engineRpc(lock, "file_index.scope_add", { path: fixtureRoot })).ok).toBe(true);
    expect(
      (
        await engineRpc(lock, "file_index.ignore_add", {
          pattern: "node_modules",
        })
      ).ok,
    ).toBe(true);
    for (let i = 0; i < 30; i++) {
      const diagnostics = await engineRpc(lock, "file_index.diagnostics");
      // SAFETY: diagnostics data is the Engine file-index diagnostics payload.
      if ((diagnostics.data as { files_count?: number } | undefined)?.files_count) break;
      await wait(200);
    }
    expect((await engineRpc(lock, "file_index.settings_set", { enabled: false })).ok).toBe(true);
    const seededSettings = await engineRpc(lock, "file_index.settings_get");
    const seededDiagnostics = await engineRpc(lock, "file_index.diagnostics");
    const marker = fs.readFileSync(path.join(fixtureRoot, "fixture.txt"), "utf8");
    await expect(
      launchManager(
        runRoot,
        "failed-start",
        managerEnv,
        path.join(managerRoot, "dist-electron", "missing-main.js"),
      ),
    ).rejects.toThrow();
    expect(fs.readFileSync(path.join(fixtureRoot, "fixture.txt"), "utf8")).toBe(marker);
    expect((await engineRpc(lock, "file_index.settings_get")).data).toEqual(seededSettings.data);
    expect((await engineRpc(lock, "file_index.diagnostics")).data).toEqual(seededDiagnostics.data);

    manager = await launchManager(runRoot, "first", managerEnv);
    pids.add(manager.process().pid);
    const page = await manager.firstWindow();
    const browserMessages: string[] = [];
    page.on("console", (message) => browserMessages.push(message.text()));
    page.on("pageerror", (error) => browserMessages.push(error.message));
    await expect(page.locator("[aria-label='Разделы менеджера']")).toBeVisible();
    await page.reload();
    await expect(page.locator("[aria-label='Разделы менеджера']")).toBeVisible();
    await expect(page.getByRole("button", { name: /Индекс файлов/ })).toHaveCount(0);
    expect(await page.evaluate(() => window.kosmosManager.getFileIndexSettings())).toMatchObject({
      ok: true,
      data: { enabled: false, ignore_patterns: ["node_modules"] },
    });
    expect(
      await page.evaluate(() =>
        window.kosmosManager.setFileIndexSettings({ include_hidden: true }),
      ),
    ).toMatchObject({
      ok: true,
      data: { enabled: false, include_hidden: true },
    });
    const nestedRoot = path.join(fixtureRoot, "nested");
    fs.mkdirSync(nestedRoot, { recursive: true });
    expect(
      await page.evaluate(
        (value) => window.kosmosManager.addFileIndexRoot({ path: value }),
        nestedRoot,
      ),
    ).toMatchObject({ ok: true });
    expect(
      await page.evaluate(() => window.kosmosManager.addFileIndexIgnore({ pattern: "dist" })),
    ).toMatchObject({
      ok: true,
    });
    expect(
      await page.evaluate(() => window.kosmosManager.removeFileIndexIgnore({ pattern: "dist" })),
    ).toMatchObject({
      ok: true,
    });
    expect(await page.evaluate(() => window.kosmosManager.getFileIndexDiagnostics())).toMatchObject(
      { ok: true, data: { scan_in_progress: false } },
    );
    expect(await page.evaluate(() => window.kosmosManager.pickFileIndexRoot())).toEqual({
      ok: true,
      data: null,
    });
    expect(await page.evaluate(() => window.kosmosManager.clearFileIndexCache())).toMatchObject({
      ok: true,
    });
    expect(await page.evaluate(() => window.kosmosManager.getFileIndexSettings())).toMatchObject({
      ok: true,
      data: {
        enabled: false,
        include_hidden: true,
        ignore_patterns: ["node_modules"],
      },
    });
    expect(browserMessages.join("\n")).not.toMatch(/auth_token|engine\.lock|password|secret/i);
    expect(await page.evaluate(() => window.kosmosManager.getFileIndexDiagnostics())).toMatchObject(
      {
        ok: true,
        data: { files_count: 0 },
      },
    );
    expect(await engineRpc(lock, "file_index.settings_get")).toMatchObject({
      ok: true,
      data: {
        enabled: false,
        include_hidden: true,
        ignore_patterns: ["node_modules"],
      },
    });
    expect(
      await manager.evaluate(({ BrowserWindow }) =>
        BrowserWindow.getAllWindows().map((window) => ({
          visible: window.isVisible(),
          focused: window.isFocused(),
        })),
      ),
    ).toEqual([{ visible: false, focused: false }]);
    await closeHost(manager, pids);
    manager = undefined;
    reopened = await launchManager(runRoot, "reopen", managerEnv);
    pids.add(reopened.process().pid);
    expect(
      await (
        await reopened.firstWindow()
      ).evaluate(() => window.kosmosManager.getFileIndexSettings()),
    ).toMatchObject({
      ok: true,
      data: {
        enabled: false,
        include_hidden: true,
        ignore_patterns: ["node_modules"],
      },
    });
  } finally {
    const cleanupErrors: unknown[] = [];
    const attempt = async (action: () => Promise<void>) => {
      try {
        await action();
      } catch (error) {
        cleanupErrors.push(error);
      }
    };
    await attempt(() => closeHost(reopened, pids));
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
    expect(cleanupErrors, "file index E2E cleanup failed").toHaveLength(0);
  }
});
