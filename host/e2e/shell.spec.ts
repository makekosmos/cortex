import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ArkClient } from "@kosmos/ark";
import { expect, test, type ElectronApplication } from "@playwright/test";
import { _electron as electron, chromium, type Browser } from "playwright";
import electronBinary from "electron";
import { createSignedApps } from "./fixtures/signed-apps";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");
type Lock = { pid: number; http_port: number; auth_token: string };

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const alive = (pid: number) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};
const waitFor = async <T>(read: () => T | undefined, label: string) => {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const value = read();
    if (value !== undefined) return value;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${label}`);
};
const gone = async (pid: number, label: string) => {
  const deadline = Date.now() + 10_000;
  while (alive(pid) && Date.now() < deadline) await sleep(100);
  if (alive(pid)) throw new Error(`${label} PID remains alive`);
};
const target = () =>
  JSON.parse(
    execFileSync("cargo", ["metadata", "--no-deps", "--format-version=1"], {
      cwd: repositoryRoot,
      encoding: "utf8",
    }),
  ).target_directory as string;
const buildEngine = (trust: { root: string; releases: string }) => {
  const env = {
    ...process.env,
    KOSMOS_PACKAGE_ROOT_KEY_JSON: trust.root,
    KOSMOS_PACKAGE_RELEASE_KEYS_JSON: trust.releases,
  };
  execFileSync("cargo", ["build", "-p", "ark-core", "--bin", "ark-core-rpc"], {
    cwd: repositoryRoot,
    env,
    stdio: "inherit",
  });
  execFileSync("cargo", ["build", "-p", "kepler-backend", "--bin", "kepler-backend"], {
    cwd: repositoryRoot,
    env,
    stdio: "inherit",
  });
  return {
    engine: path.join(target(), "debug", "kepler-backend.exe"),
    ark: path.join(target(), "debug", "ark-core-rpc.exe"),
  };
};
const startEngine = async (engine: string, ark: string, dataDir: string) => {
  const child = spawn(engine, [], {
    env: {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      ARK_CORE_RPC_PATH: ark,
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KEPLER_SKIP_SYNC: "1",
      KEPLER_USAGE_TRACKER: "0",
    },
    stdio: "ignore",
    windowsHide: true,
  });
  const lock = await waitFor(() => {
    try {
      return JSON.parse(fs.readFileSync(path.join(dataDir, "engine.lock.json"), "utf8")) as Lock;
    } catch {
      return undefined;
    }
  }, "Engine lock");
  return { child, lock };
};
const rpc = async (lock: Lock, operation: string, params: Record<string, unknown> = {}) => {
  const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lock.auth_token}`,
      Connection: "close",
      "Content-Type": "application/json",
      "X-Kosmos-Api-Version": "1.0.0",
      "X-Kosmos-Client-Class": "desktop-host",
      "X-Kosmos-Client-Version": "1.0.0",
      "X-Kosmos-Client-Pid": String(process.pid),
    },
    body: JSON.stringify({ operation, _req_id: randomUUID(), ...params }),
    signal: AbortSignal.timeout(20_000),
  });
  return response.json() as Promise<{ ok: boolean; data?: unknown }>;
};
const invoke = (id: string, userData: string, env: NodeJS.ProcessEnv) =>
  new Promise<number | null>((resolve, reject) => {
    const child = spawn(
      electronBinary,
      [`--user-data-dir=${userData}`, hostMain, `--open-app=${id}`],
      { env, windowsHide: true, stdio: "ignore" },
    );
    child.once("error", reject);
    child.once("exit", resolve);
  });
const closeHost = async (host: ElectronApplication | undefined) => {
  if (!host) return;
  const pid = host.process().pid;
  await Promise.race([host.close().catch(() => undefined), sleep(5_000)]);
  if (alive(pid)) {
    try {
      execFileSync("taskkill.exe", ["/PID", String(pid), "/T", "/F"], {
        windowsHide: true,
        stdio: "ignore",
        timeout: 5_000,
      });
    } catch {}
  }
  await gone(pid, "Host");
};
const stopEngine = async (child: ChildProcess | undefined, engine: string, dataDir: string) => {
  if (!child?.pid || !alive(child.pid)) return;
  try {
    execFileSync(engine, ["--shutdown"], {
      env: { ...process.env, KOSMOS_DATA_DIR: dataDir, KOSMOS_LOCK_PERMISSIONS_DISABLED: "1" },
      windowsHide: true,
      stdio: "ignore",
      timeout: 10_000,
    });
  } catch {}
  if (alive(child.pid)) {
    try {
      execFileSync("taskkill.exe", ["/PID", String(child.pid), "/T", "/F"], {
        windowsHide: true,
        stdio: "ignore",
        timeout: 5_000,
      });
    } catch {}
  }
  await gone(child.pid, "Engine");
};

test("signed Shell is capability-scoped, warm-reopens, and leaves Graph available after removal", async () => {
  test.setTimeout(180_000);
  test.skip(!fs.existsSync(hostMain), `build Host first: ${hostMain}`);
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-"));
  const dataDir = path.join(root, "engine");
  const userData = path.join(root, "host-user-data");
  const shortcutDir = path.join(root, "shortcuts");
  const searchRoot = path.join(root, "search");
  let host: ElectronApplication | undefined;
  let engine: ChildProcess | undefined;
  let commandClient: ArkClient | undefined;
  let visual: Browser | undefined;
  let lock: Lock;
  const pids = new Set<number>();
  const manifestPath = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!manifestPath) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");
  try {
    fs.mkdirSync(searchRoot, { recursive: true });
    fs.writeFileSync(path.join(searchRoot, "shell-fixture.md"), "Shell fixture", "utf8");
    const apps = createSignedApps(root, repositoryRoot, true, true);
    const binaries = buildEngine(apps.trust);
    ({ child: engine, lock } = await startEngine(binaries.engine, binaries.ark, dataDir));
    if (engine.pid) pids.add(engine.pid);
    const enginePid = lock.pid;
    expect(
      (
        await rpc(lock, "packages.catalog_apply", {
          document: apps.catalog,
          signatures: apps.signatures,
        })
      ).ok,
    ).toBe(true);
    for (const [id, archive] of Object.entries(apps.archives)) {
      const version = apps.versions[id];
      expect(
        (await rpc(lock, "packages.install", { id, version, archive_path: archive })).ok,
        id,
      ).toBe(true);
      expect((await rpc(lock, "packages.set_enabled", { id, version, enabled: true })).ok, id).toBe(
        true,
      );
    }
    expect((await rpc(lock, "file_index.scope_add", { path: searchRoot })).ok).toBe(true);
    await expect
      .poll(
        async () => {
          const response = await rpc(lock, "file_index.search", { query: "shell-fixture" });
          return (response.data as { results?: unknown[] } | undefined)?.results?.length ?? 0;
        },
        { timeout: 30_000 },
      )
      .toBeGreaterThan(0);
    expect((await rpc(lock, "engine.settings.set", { warm_timeout_seconds: 300 })).ok).toBe(true);
    commandClient = new ArkClient({
      spaceId: "shell-e2e",
      deviceId: `shell-e2e-${process.pid}`,
      engineLock: lock,
      engineClientClass: "desktop-host",
      engineClientVersion: "1.0.0",
    });
    await commandClient.start();
    await commandClient.invokeOperation({
      operation: "commands.register",
      commands: [{ id: "shell.fixture", title: "Команда Shell", category: "action" }],
    });
    const environment = {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_SHORTCUT_DIR: shortcutDir,
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
    };
    host = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.shell"],
      env: environment,
      timeout: 30_000,
    });
    pids.add(host.process().pid);
    let page = await host.firstWindow();
    expect(await page.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.shell",
      version: "0.2.0",
    });
    expect(await page.evaluate(() => window.kosmosApp.ark.request("list_objects"))).toMatchObject({
      ok: false,
    });
    expect(
      await page.evaluate(() => window.kosmosApp.launcher.request("packages.list")),
    ).toMatchObject({ ok: false });
    expect(
      await page.evaluate(() =>
        window.kosmosApp.launcher.request("app_index.search", { query: "Shell", limit: 8 }),
      ),
    ).toMatchObject({ ok: true });
    await expect(page.locator("input")).toBeVisible();
    await page.locator("input").fill("shell-fixture");
    await expect(page.getByText("shell-fixture.md")).toBeVisible();
    await page.locator("input").fill("Shell");
    await expect(page.getByText("Команда Shell")).toBeVisible();
    const firstSelected = await page.locator("button.selected").innerText();
    await page.locator("input").press("ArrowDown");
    await expect.poll(() => page.locator("button.selected").innerText()).not.toBe(firstSelected);
    expect(page.isClosed(), "ArrowDown must not close the Shell renderer").toBe(false);
    expect(
      await host.evaluate(({ BrowserWindow }) =>
        BrowserWindow.getAllWindows().map((window) => ({
          destroyed: window.isDestroyed(),
          visible: window.isVisible(),
        })),
      ),
    ).toEqual([{ destroyed: false, visible: false }]);
    expect(alive(host.process().pid), "ArrowDown must not stop the warm Host").toBe(true);
    expect(
      await host.evaluate(({ BrowserWindow }) => {
        const shell = BrowserWindow.getAllWindows()[0];
        if (!shell) return false;
        shell.webContents.sendInputEvent({ type: "keyDown", keyCode: "Escape" });
        return true;
      }),
    ).toBe(true);
    await expect.poll(() => host!.windows().length).toBe(0);
    expect(alive(host.process().pid)).toBe(true);
    expect(
      (JSON.parse(fs.readFileSync(path.join(dataDir, "engine.lock.json"), "utf8")) as Lock).pid,
    ).toBe(enginePid);
    expect(await invoke("com.kosmos.shell", userData, environment)).toBe(0);
    await expect.poll(() => host!.windows().length).toBe(1);
    page = host.windows()[0];
    expect(await invoke("com.kosmos.shell", userData, environment)).toBe(0);
    await expect.poll(() => host!.windows().length).toBe(1);
    await page.getByText("Команда Shell").click();
    await expect.poll(() => host!.windows().length).toBe(0);
    console.log(
      "[shell-e2e] signed search, keyboard, command invoke, capability denial, and warm reopen verified",
    );

    const shellLaunch = (await fetch(`http://127.0.0.1:${lock.http_port}/v1/apps/launch`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${lock.auth_token}`,
        "Content-Type": "application/json",
        "X-Kosmos-Api-Version": "1.0.0",
        "X-Kosmos-Client-Class": "desktop-host",
        "X-Kosmos-Client-Version": "1.0.0",
        "X-Kosmos-Client-Pid": String(process.pid),
      },
      body: JSON.stringify({ id: "com.kosmos.shell" }),
    }).then((response) => response.json())) as { data?: { launch_url?: string } };
    visual = await chromium.launch({ headless: true });
    const visualPage = await visual.newPage({ viewport: { width: 1000, height: 760 } });
    await visualPage.addInitScript(() => {
      window.kosmosApp = {
        window: { close: () => undefined, minimize: () => undefined },
        launcher: {
          request: async (operation: string) => ({
            ok: true,
            data:
              operation === "commands.list"
                ? { commands: [{ id: "shell.fixture", title: "Команда Shell" }] }
                : operation === "app_index.list_all"
                  ? { apps: [] }
                  : { results: [] },
          }),
        },
        identity: {},
      } as unknown as typeof window.kosmosApp;
    });
    await visualPage.goto(shellLaunch.data!.launch_url!, { waitUntil: "networkidle" });
    await expect(visualPage.getByText("Команда Shell")).toBeVisible();
    const visualRoot = path.join(
      repositoryRoot,
      ".tmp",
      "visual",
      "2026-07-29-shell-app-extraction",
    );
    fs.mkdirSync(visualRoot, { recursive: true });
    await visualPage.screenshot({ path: path.join(visualRoot, "shell-1000x760-default.png") });
    await visual.close();
    visual = undefined;

    await commandClient.stop();
    commandClient = undefined;
    await closeHost(host);
    host = undefined;
    expect(
      (
        await rpc(lock, "packages.set_enabled", {
          id: "com.kosmos.shell",
          version: "0.2.0",
          enabled: false,
        })
      ).ok,
    ).toBe(true);
    expect(
      (await rpc(lock, "packages.uninstall", { id: "com.kosmos.shell", version: "0.2.0" })).ok,
    ).toBe(true);
    host = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.graph"],
      env: environment,
      timeout: 30_000,
    });
    pids.add(host.process().pid);
    await expect.poll(() => host!.windows().length).toBe(1);
    const graphPage = await host.firstWindow();
    expect(await graphPage.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.graph",
    });
    expect(await invoke("host-e2e-app-b", userData, environment)).toBe(0);
    await expect.poll(() => host!.windows().length).toBe(2);
    expect(fs.existsSync(path.join(shortcutDir, "Kosmos Shell.lnk"))).toBe(false);
    expect(fs.existsSync(path.join(shortcutDir, "Мой космос.lnk"))).toBe(true);
    expect(fs.existsSync(path.join(shortcutDir, "host-e2e-app-b.lnk"))).toBe(true);
    expect((await rpc(lock, "list_object_types")).ok).toBe(true);
    expect(
      (JSON.parse(fs.readFileSync(path.join(dataDir, "engine.lock.json"), "utf8")) as Lock).pid,
    ).toBe(enginePid);
    console.log(
      "[shell-e2e] Shell-absent Graph, fixture shortcut, and Engine/ARK continuity verified",
    );
  } finally {
    await visual?.close().catch(() => undefined);
    await commandClient?.stop().catch(() => undefined);
    await closeHost(host);
    await stopEngine(engine, path.join(target(), "debug", "kepler-backend.exe"), dataDir);
    for (const pid of pids) await gone(pid, "recorded process");
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as {
      roots: string[];
      pids: number[];
    };
    manifest.roots.push(root);
    manifest.pids.push(...pids);
    fs.writeFileSync(manifestPath, JSON.stringify(manifest));
  }
});
