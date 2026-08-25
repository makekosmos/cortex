import fs from "node:fs";
import path from "node:path";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const runRoot = path.join(
  root,
  ".e2e",
  "file-index",
  `${process.pid}-${Date.now()}`,
);
const dataDir = path.join(runRoot, "data");
const fixtureRoot = path.join(runRoot, "fixture-root");
const lockPath = path.join(dataDir, "engine.lock.json");
const engineBinary = path.resolve(
  root,
  "..",
  "..",
  "target",
  "debug",
  "kepler-backend.exe",
);
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
function removeRunRoot() {
  const relative = path.relative(root, runRoot);
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative))
    throw new Error("invalid isolated test root");
  fs.rmSync(runRoot, {
    recursive: true,
    force: true,
    maxRetries: 5,
    retryDelay: 200,
  });
}

function readLock() {
  try {
    // SAFETY: the fixture lock file is written by the Engine launcher with this schema.
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
async function waitLock() {
  for (let i = 0; i < 150; i++) {
    const lock = readLock();
    if (lock) return lock;
    await wait(200);
  }
  throw new Error("Engine lock timeout");
}
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
function launch(slot: string, entry = "main.js"): Promise<ElectronApplication> {
  const userData = path.join(runRoot, slot, "userdata");
  fs.mkdirSync(userData, { recursive: true });
  return electron.launch({
    executablePath: electronBinary,
    cwd: root,
    args: [
      `--user-data-dir=${userData}`,
      path.join(root, "dist-electron", entry),
    ],
    env: {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      NODE_ENV: "test",
    },
    timeout: 30_000,
  });
}

test("Manager keeps the File Index API without a sidebar surface", async () => {
  removeRunRoot();
  fs.mkdirSync(fixtureRoot, { recursive: true });
  fs.writeFileSync(path.join(fixtureRoot, "fixture.txt"), "fixture");
  let engine: ChildProcess | undefined;
  let manager: ElectronApplication | undefined;
  let reopened: ElectronApplication | undefined;
  try {
    engine = spawn(engineBinary, [], {
      cwd: root,
      stdio: "ignore",
      windowsHide: true,
      env: {
        ...process.env,
        KOSMOS_DATA_DIR: dataDir,
        KOSMOS_TEST_MODE: "1",
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
        KEPLER_SKIP_SYNC: "1",
      },
    });
    const lock = await waitLock();
    expect(
      (
        await engineRpc(lock, "file_index.settings_set", {
          enabled: true,
          include_hidden: false,
        })
      ).ok,
    ).toBe(true);
    expect(
      (await engineRpc(lock, "file_index.scope_add", { path: fixtureRoot })).ok,
    ).toBe(true);
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
      if (
        (diagnostics.data as { files_count?: number } | undefined)?.files_count
      )
        break;
      await wait(200);
    }
    expect(
      (await engineRpc(lock, "file_index.settings_set", { enabled: false })).ok,
    ).toBe(true);
    const seededSettings = await engineRpc(lock, "file_index.settings_get");
    const seededDiagnostics = await engineRpc(lock, "file_index.diagnostics");
    const marker = fs.readFileSync(
      path.join(fixtureRoot, "fixture.txt"),
      "utf8",
    );
    await expect(launch("failed-start", "missing-main.js")).rejects.toThrow();
    expect(fs.readFileSync(path.join(fixtureRoot, "fixture.txt"), "utf8")).toBe(
      marker,
    );
    expect((await engineRpc(lock, "file_index.settings_get")).data).toEqual(
      seededSettings.data,
    );
    expect((await engineRpc(lock, "file_index.diagnostics")).data).toEqual(
      seededDiagnostics.data,
    );

    manager = await launch("first");
    const page = await manager.firstWindow();
    const browserMessages: string[] = [];
    page.on("console", (message) => browserMessages.push(message.text()));
    page.on("pageerror", (error) => browserMessages.push(error.message));
    await page.reload();
    await expect(
      page.getByRole("button", { name: /Индекс файлов/ }),
    ).toHaveCount(0);
    expect(
      await page.evaluate(() => window.kosmosManager.getFileIndexSettings()),
    ).toMatchObject({
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
      await page.evaluate(() =>
        window.kosmosManager.addFileIndexIgnore({ pattern: "dist" }),
      ),
    ).toMatchObject({
      ok: true,
    });
    expect(
      await page.evaluate(() =>
        window.kosmosManager.removeFileIndexIgnore({ pattern: "dist" }),
      ),
    ).toMatchObject({
      ok: true,
    });
    expect(
      await page.evaluate(() => window.kosmosManager.getFileIndexDiagnostics()),
    ).toMatchObject({ ok: true, data: { scan_in_progress: false } });
    expect(
      await page.evaluate(() => window.kosmosManager.pickFileIndexRoot()),
    ).toEqual({
      ok: true,
      data: null,
    });
    expect(
      await page.evaluate(() => window.kosmosManager.clearFileIndexCache()),
    ).toMatchObject({
      ok: true,
    });
    expect(
      await page.evaluate(() => window.kosmosManager.getFileIndexSettings()),
    ).toMatchObject({
      ok: true,
      data: {
        enabled: false,
        include_hidden: true,
        ignore_patterns: ["node_modules"],
      },
    });
    expect(browserMessages.join("\n")).not.toMatch(
      /auth_token|engine\.lock|password|secret/i,
    );
    expect(
      await page.evaluate(() => window.kosmosManager.getFileIndexDiagnostics()),
    ).toMatchObject({
      ok: true,
      data: { files_count: 0 },
    });
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
    await manager.close();
    manager = undefined;
    reopened = await launch("reopen");
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
        windowsHide: true,
      });
      engine.kill();
    }
    await wait(500);
    removeRunRoot();
  }
});
