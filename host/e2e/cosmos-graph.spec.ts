import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test, expect, type ElectronApplication, type Page } from "@playwright/test";
import { _electron as electron, chromium, type Browser } from "playwright";
import electronBinary from "electron";
import { createSignedApps } from "./fixtures/signed-apps";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");
type Lock = { pid: number; http_port: number; auth_token: string };

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const waitFor = async <T>(read: () => T | undefined, label: string): Promise<T> => {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const value = read();
    if (value !== undefined) return value;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${label}`);
};
const cargoTarget = () =>
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
  const target = cargoTarget();
  return {
    engine: path.join(target, "debug", "kepler-backend.exe"),
    ark: path.join(target, "debug", "ark-core-rpc.exe"),
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
const launch = async (lock: Lock) => {
  const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/apps/launch`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lock.auth_token}`,
      "Content-Type": "application/json",
      "X-Kosmos-Api-Version": "1.0.0",
      "X-Kosmos-Client-Class": "desktop-host",
      "X-Kosmos-Client-Version": "1.0.0",
      "X-Kosmos-Client-Pid": String(process.pid),
    },
    body: JSON.stringify({ id: "com.kosmos.graph" }),
  });
  return response.json() as Promise<{ ok: boolean; data?: { launch_url?: string } }>;
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
const alive = (pid: number) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};
const gone = async (pid: number, label: string) => {
  const deadline = Date.now() + 10_000;
  while (alive(pid) && Date.now() < deadline) await sleep(100);
  if (alive(pid)) throw new Error(`${label} PID ${pid} is still alive`);
};
const stop = async (child: ChildProcess | undefined, engine: string, dataDir: string) => {
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
const closeHost = async (host: ElectronApplication | undefined) => {
  if (!host) return;
  let pid: number;
  try {
    pid = host.process().pid;
  } catch {
    return;
  }
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
const capture = async (page: Page, width: number, height: number) => {
  await page.setViewportSize({ width, height });
  await expect
    .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth))
    .toBe(true);
  const root = path.join(repositoryRoot, ".tmp", "visual", "2026-07-29-cosmos-graph-app");
  fs.mkdirSync(root, { recursive: true });
  await page.screenshot({ path: path.join(root, `${width}x${height}.png`) });
};

test("signed Graph App uses only scoped ARK reads and survives warm reopen", async () => {
  test.setTimeout(180_000);
  test.skip(!fs.existsSync(hostMain), `build Host first: ${hostMain}`);
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-"));
  const dataDir = path.join(root, "engine");
  const userData = path.join(root, "host-user-data");
  let host: ElectronApplication | undefined;
  let engine: ChildProcess | undefined;
  let visualBrowser: Browser | undefined;
  let lock: Lock;
  const pids = new Set<number>();
  const manifestPath = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!manifestPath) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");
  try {
    const apps = createSignedApps(root, repositoryRoot, true);
    const graphVersion = apps.versions["com.kosmos.graph"];
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
    expect(
      (
        await rpc(lock, "packages.install", {
          id: "com.kosmos.graph",
          version: graphVersion,
          archive_path: apps.archives["com.kosmos.graph"],
        })
      ).ok,
    ).toBe(true);
    expect(
      (
        await rpc(lock, "packages.set_enabled", {
          id: "com.kosmos.graph",
          version: graphVersion,
          enabled: true,
        })
      ).ok,
    ).toBe(true);
    const graphLaunch = await launch(lock);
    expect(graphLaunch).toMatchObject({ ok: true, data: { launch_url: expect.any(String) } });
    expect((await fetch(graphLaunch.data!.launch_url!)).ok).toBe(true);
    const at = "2026-07-01T00:00:00.000Z";
    for (const [operation, params] of [
      [
        "upsert_object_type",
        {
          object_type: {
            id: "graph_private_type",
            name: "Private fixture",
            schemaJson: "{}",
            uiSchemaJson: "{}",
            createdAt: at,
            updatedAt: at,
            systemLocked: false,
          },
          device_id: "host-e2e",
        },
      ],
      [
        "upsert_object",
        {
          object: {
            id: "graph-private",
            typeId: "graph_private_type",
            title: "Private fixture",
            contentJson: {},
            propsJson: {},
            createdAt: at,
            updatedAt: at,
            deletedAt: null,
          },
          device_id: "host-e2e",
        },
      ],
      [
        "upsert_object",
        {
          object: {
            id: "graph-fixture-a",
            typeId: "com.kosmos.note",
            typeVersion: "1.0.0",
            title: "Graph fixture A",
            contentJson: { type: "doc", content: [] },
            propsJson: { description: null, extensions: {} },
            createdAt: at,
            updatedAt: at,
            deletedAt: null,
          },
          device_id: "host-e2e",
        },
      ],
      [
        "upsert_object",
        {
          object: {
            id: "graph-fixture-b",
            typeId: "com.kosmos.note",
            typeVersion: "1.0.0",
            title: "Graph fixture B",
            contentJson: { type: "doc", content: [] },
            propsJson: { description: null, extensions: {} },
            createdAt: at,
            updatedAt: at,
            deletedAt: null,
          },
          device_id: "host-e2e",
        },
      ],
      [
        "upsert_object_link",
        {
          object_link: {
            id: "graph-fixture-link",
            sourceObjectId: "graph-fixture-a",
            targetObjectId: "graph-fixture-b",
            linkType: "related",
            createdAt: at,
          },
          device_id: "host-e2e",
        },
      ],
    ] as const)
      expect((await rpc(lock, operation, params)).ok, operation).toBe(true);
    expect((await rpc(lock, "engine.settings.set", { warm_timeout_seconds: 300 })).ok).toBe(true);
    const environment = {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
    };
    host = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.graph"],
      env: environment,
      timeout: 30_000,
    });
    pids.add(host.process().pid);
    const page = await host.firstWindow();
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    expect(await page.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.graph",
      version: graphVersion,
    });
    await sleep(1_000);
    const graphState = await page.evaluate(() => ({
      canvas: document.querySelector("canvas") !== null,
      text: document.body.innerText,
    }));
    expect(graphState.canvas, `${graphState.text}\n${errors.join("\n")}`).toBe(true);
    console.log("[graph-e2e] Host graph rendered");
    const reads = await page.evaluate(async () => ({
      types: await window.kosmosApp.ark.request("list_object_types"),
      objects: await window.kosmosApp.ark.request("list_objects"),
      links: await window.kosmosApp.ark.request("list_object_links"),
      denied: await window.kosmosApp.ark.request("get_object", { id: "graph-private" }),
    }));
    expect(reads.types).toMatchObject({
      ok: true,
      data: expect.arrayContaining([expect.objectContaining({ id: "com.kosmos.note" })]),
    });
    expect(reads.objects).toMatchObject({
      ok: true,
      data: expect.arrayContaining([expect.objectContaining({ id: "graph-fixture-a" })]),
    });
    expect(reads.links).toMatchObject({
      ok: true,
      data: expect.arrayContaining([expect.objectContaining({ id: "graph-fixture-link" })]),
    });
    expect(reads.denied).toMatchObject({ ok: false });
    expect(errors).toEqual([]);
    console.log("[graph-e2e] Host scoped reads verified");
    visualBrowser = await chromium.launch({
      headless: true,
      args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
    });
    const visualPage = await visualBrowser.newPage();
    const visualErrors: string[] = [];
    visualPage.on("pageerror", (error) => visualErrors.push(error.message));
    visualPage.on("console", (message) => {
      if (message.type() === "error") visualErrors.push(message.text());
    });
    await visualPage.addInitScript(() => {
      const types = [{ id: "com.kosmos.note", name: "Заметка" }];
      const objects = [
        { id: "graph-fixture-a", typeId: "com.kosmos.note", title: "Graph fixture A" },
        { id: "graph-fixture-b", typeId: "com.kosmos.note", title: "Graph fixture B" },
      ];
      const links = [
        {
          id: "graph-fixture-link",
          sourceObjectId: "graph-fixture-a",
          targetObjectId: "graph-fixture-b",
          linkType: "related",
          createdAt: "2026-07-01T00:00:00.000Z",
        },
      ];
      window.kosmosApp = {
        ark: {
          request: async (operation: string) => ({
            ok: true,
            data:
              operation === "list_object_types"
                ? types
                : operation === "list_objects"
                  ? objects
                  : operation === "list_object_links"
                    ? links
                    : [],
          }),
          subscribe: () => () => undefined,
        },
      } as typeof window.kosmosApp;
    });
    await visualPage.goto(graphLaunch.data!.launch_url!, { waitUntil: "networkidle" });
    await expect
      .poll(() => visualPage.evaluate(() => document.querySelector("canvas") !== null))
      .toBe(true);
    console.log("[graph-e2e] visual renderer ready");
    await capture(visualPage, 1440, 900);
    await capture(visualPage, 1024, 720);
    expect(visualErrors).toEqual([]);
    await visualBrowser.close();
    visualBrowser = undefined;
    expect(await invoke("com.kosmos.graph", userData, environment)).toBe(0);
    await expect.poll(() => host!.windows().length).toBe(1);
    const scheduledClose = await host.evaluate(({ BrowserWindow }) => {
      const win = BrowserWindow.getAllWindows()[0];
      if (!win) return false;
      setTimeout(() => win.destroy(), 100);
      return true;
    });
    expect(scheduledClose).toBe(true);
    await expect.poll(() => host!.windows().length).toBe(0);
    expect(alive(host.process().pid)).toBe(true);
    expect(
      (JSON.parse(fs.readFileSync(path.join(dataDir, "engine.lock.json"), "utf8")) as Lock).pid,
    ).toBe(enginePid);
    expect(await invoke("com.kosmos.graph", userData, environment)).toBe(0);
    await expect.poll(() => host!.windows().length).toBe(1);
    console.log(
      "[graph-e2e] signed scoped read, two viewports, warm reopen, and Engine PID verified",
    );
  } finally {
    await visualBrowser?.close().catch(() => undefined);
    await closeHost(host);
    await stop(engine, path.join(cargoTarget(), "debug", "kepler-backend.exe"), dataDir);
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
