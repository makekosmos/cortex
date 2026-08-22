import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";
import { createSignedApps } from "./fixtures/signed-apps";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");

type Lock = { pid: number; http_port: number; auth_token: string };

const waitFor = async <T>(read: () => T | undefined, label: string): Promise<T> => {
  const until = Date.now() + 30_000;
  while (Date.now() < until) {
    const value = read();
    if (value !== undefined) return value;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`timed out waiting for ${label}`);
};

const cargoTarget = (): string =>
  JSON.parse(
    execFileSync("cargo", ["metadata", "--no-deps", "--format-version=1"], {
      cwd: repositoryRoot,
      encoding: "utf8",
    }),
  ).target_directory;

const buildEngine = (trust: {
  root: string;
  releases: string;
}): { engine: string; ark: string } => {
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
  execFileSync("cargo", ["build", "-p", "kepler-backend"], {
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

const startEngine = async (
  engine: string,
  ark: string,
  dataDir: string,
): Promise<{ child: ChildProcess; lock: Lock }> => {
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
  console.log(`[host-e2e] rpc start operation=${operation}`);
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
  const result = (await response.json()) as { ok: boolean; data?: unknown; error?: unknown };
  console.log(
    `[host-e2e] rpc result operation=${operation} ok=${result.ok} error=${rpcError(result)}`,
  );
  return result;
};

const rpcError = (result: { error?: unknown }): string =>
  typeof result.error === "string" ? result.error.slice(0, 256) : "unknown error";

const invoke = (id: string, userData: string, env: NodeJS.ProcessEnv): Promise<number | null> =>
  new Promise((resolve, reject) => {
    const child = spawn(
      electronBinary,
      [`--user-data-dir=${userData}`, hostMain, `--open-app=${id}`],
      { env, windowsHide: true, stdio: "ignore" },
    );
    child.once("error", reject);
    child.once("exit", (code) => resolve(code));
  });

const isPidAlive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

const waitForPidGone = async (pid: number, label: string): Promise<void> => {
  const deadline = Date.now() + 10_000;
  while (isPidAlive(pid) && Date.now() < deadline)
    await new Promise((resolve) => setTimeout(resolve, 100));
  if (isPidAlive(pid)) throw new Error(`${label} PID ${pid} is still alive`);
};

const forceStop = async (pid: number, label: string): Promise<void> => {
  try {
    execFileSync("taskkill.exe", ["/PID", String(pid), "/T", "/F"], {
      windowsHide: true,
      stdio: "ignore",
      timeout: 5_000,
    });
  } catch {}
  await waitForPidGone(pid, label);
};

const terminate = async (
  child: ChildProcess | undefined,
  engine: string,
  dataDir: string,
  label: string,
): Promise<void> => {
  const pid = child?.pid;
  if (!pid || !isPidAlive(pid)) return;
  console.log(
    `[host-e2e] teardown ${label}: pid=${pid} lock=${path.join(dataDir, "engine.lock.json")}`,
  );
  try {
    execFileSync(engine, ["--shutdown"], {
      env: { ...process.env, KOSMOS_DATA_DIR: dataDir, KOSMOS_LOCK_PERMISSIONS_DISABLED: "1" },
      windowsHide: true,
      stdio: "ignore",
      timeout: 10_000,
    });
  } catch {}
  if (isPidAlive(pid)) await forceStop(pid, label);
  await waitForPidGone(pid, label);
};

const closeHost = async (host: ElectronApplication | undefined): Promise<void> => {
  if (!host) return;
  const pid = host.process().pid;
  console.log(`[host-e2e] teardown Host: pid=${pid}`);
  await Promise.race([
    host.close().catch(() => undefined),
    new Promise((resolve) => setTimeout(resolve, 5_000)),
  ]);
  if (isPidAlive(pid)) await forceStop(pid, "Host");
  await waitForPidGone(pid, "Host");
};

const recordCleanup = (manifestPath: string, root: string, pids: Set<number>): void => {
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as {
    roots?: unknown;
    pids?: unknown;
  };
  if (!Array.isArray(manifest.roots) || !Array.isArray(manifest.pids))
    throw new Error("invalid Host E2E cleanup manifest");
  manifest.roots.push(root);
  manifest.pids.push(...pids);
  fs.writeFileSync(manifestPath, JSON.stringify(manifest));
};

test("real signed Apps use one headless Host and preserve the Engine across restart", async () => {
  test.setTimeout(180_000);
  test.skip(!fs.existsSync(hostMain), `build Host first: ${hostMain}`);
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-"));
  const dataDir = path.join(root, "engine");
  const userData = path.join(root, "host-user-data");
  const appData = path.join(root, "ambient-appdata-sentinel");
  let host: ElectronApplication | undefined;
  let engine: ChildProcess | undefined;
  let restarted: ChildProcess | undefined;
  const teardownPids = new Set<number>();
  const manifestPath = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!manifestPath) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");
  try {
    const apps = createSignedApps(root, repositoryRoot);
    const binaries = buildEngine(apps.trust);
    expect(fs.existsSync(binaries.engine)).toBe(true);
    expect(fs.existsSync(binaries.ark)).toBe(true);
    const initial = await startEngine(binaries.engine, binaries.ark, dataDir);
    engine = initial.child;
    if (engine.pid) teardownPids.add(engine.pid);
    let lock = initial.lock;
    const initialEnginePid = lock.pid;
    const trustStatus = await rpc(lock, "packages.trust_status");
    expect(trustStatus.ok, `packages.trust_status: ${rpcError(trustStatus)}`).toBe(true);
    expect(
      (trustStatus.data as { trust?: { configured?: unknown } } | undefined)?.trust?.configured,
    ).toBe(true);
    const catalogApplied = await rpc(lock, "packages.catalog_apply", {
      document: apps.catalog,
      signatures: apps.signatures,
    });
    console.log(
      `[host-e2e] catalog_apply ok=${catalogApplied.ok} error=${rpcError(catalogApplied)}`,
    );
    expect(catalogApplied.ok, `packages.catalog_apply: ${rpcError(catalogApplied)}`).toBe(true);
    for (const id of ["host-e2e-app-a", "host-e2e-app-b"]) {
      expect(
        (
          await rpc(lock, "packages.install", {
            id,
            version: "1.0.0",
            archive_path: apps.archives[id],
          })
        ).ok,
      ).toBe(true);
      expect(
        (await rpc(lock, "packages.set_enabled", { id, version: "1.0.0", enabled: true })).ok,
      ).toBe(true);
    }
    expect((await rpc(lock, "engine.settings.set", { warm_timeout_seconds: 0 })).ok).toBe(true);
    const environment = {
      ...process.env,
      APPDATA: appData,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
    };
    host = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "host-e2e-app-a"],
      env: environment,
      timeout: 30_000,
    });
    console.log("[host-e2e] Host launched");
    const first = await host.firstWindow();
    console.log("[host-e2e] first window ready");
    await expect.poll(() => host!.windows().length).toBe(1);
    expect(await first.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "host-e2e-app-a",
      version: "1.0.0",
    });
    console.log("[host-e2e] first identity verified");
    expect(await invoke("host-e2e-app-b", userData, environment)).toBe(0);
    console.log("[host-e2e] second invocation exited");
    await expect.poll(() => host!.windows().length).toBe(2);
    expect(await invoke("host-e2e-app-a", userData, environment)).toBe(0);
    await expect.poll(() => host!.windows().length).toBe(2);
    console.log("[host-e2e] window reuse verified");
    await expect.poll(() => first.evaluate(() => Array.isArray(window.__hostEvents))).toBe(true);
    const writes = await first.evaluate(async () => ({
      type: await window.kosmosApp.ark.request("upsert_object_type", {
        object_type: {
          id: "host_e2e_type",
          name: "Host E2E",
          schemaJson: "{}",
          uiSchemaJson: "{}",
          createdAt: "2026-07-01T00:00:00.000Z",
          updatedAt: "2026-07-01T00:00:00.000Z",
          systemLocked: false,
        },
        device_id: "host-e2e",
      }),
      object: await window.kosmosApp.ark.request("upsert_object", {
        object: {
          id: "host-e2e-object",
          typeId: "com.kosmos.note",
          typeVersion: "1.0.0",
          title: "Host E2E",
          createdAt: "2026-07-01T00:00:00.000Z",
          updatedAt: "2026-07-01T00:00:00.000Z",
          deletedAt: null,
        },
        device_id: "host-e2e",
      }),
    }));
    expect(writes.type).toMatchObject({ ok: false });
    expect(writes.object).toMatchObject({ ok: false });
    expect(await first.evaluate(() => window.kosmosApp.ark.request("list_objects"))).toMatchObject({
      ok: true,
    });
    console.log("[host-e2e] v2 ARK read grant and legacy write denial verified");
    const initialHostPid = host.process().pid;
    teardownPids.add(initialHostPid);
    const hostExit = new Promise<number | null>((resolve) => host!.process().once("exit", resolve));
    const scheduledWindows = await host.evaluate(({ BrowserWindow }) => {
      const windows = BrowserWindow.getAllWindows();
      setTimeout(() => windows.forEach((win) => win.destroy()), 100);
      return windows.length;
    });
    expect(scheduledWindows).toBe(2);
    console.log("[host-e2e] main-process BrowserWindow closes scheduled");
    expect(await hostExit).toBe(0);
    console.log("[host-e2e] zero-timeout Host exit verified");
    await waitForPidGone(initialHostPid, "initial Host");
    host = undefined;
    expect(fs.existsSync(path.join(dataDir, "engine.lock.json"))).toBe(true);
    expect(
      JSON.parse(fs.readFileSync(path.join(dataDir, "engine.lock.json"), "utf8")) as Lock,
    ).toMatchObject({ pid: initialEnginePid });
    expect(fs.existsSync(path.join(appData, "Kosmos", "engine.lock.json"))).toBe(false);
    const info = (await fetch(`http://127.0.0.1:${lock.http_port}/v1/info`, {
      headers: {
        Authorization: `Bearer ${lock.auth_token}`,
        "X-Kosmos-Api-Version": "1.0.0",
        "X-Kosmos-Client-Class": "desktop-host",
        "X-Kosmos-Client-Version": "1.0.0",
        "X-Kosmos-Client-Pid": String(process.pid),
      },
    }).then((response) => response.json())) as {
      protocol_usage: {
        api_v1: { connections: number };
        legacy: { connections: number };
        clients: Record<string, unknown>;
      };
    };
    expect(info.protocol_usage.api_v1.connections).toBeGreaterThan(0);
    expect(info.protocol_usage.legacy.connections).toBe(0);
    expect(info.protocol_usage.clients["api_v1:desktop-host@1.0.0"]).toBeDefined();
    await terminate(engine, binaries.engine, dataDir, "initial Engine");
    engine = undefined;
    ({ child: restarted, lock } = await startEngine(binaries.engine, binaries.ark, dataDir));
    if (restarted.pid) teardownPids.add(restarted.pid);
    expect(lock.pid).not.toBe(initialEnginePid);
    host = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "host-e2e-app-a"],
      env: environment,
      timeout: 30_000,
    });
    await expect.poll(() => host!.windows().length).toBe(1);
    console.log("[host-e2e] Engine restart and Host reopen verified");
  } finally {
    if (host) teardownPids.add(host.process().pid);
    console.log(
      `[host-e2e] cleanup root=${root} lock=${path.join(dataDir, "engine.lock.json")} host=${host?.process().pid ?? "none"} restarted-engine=${restarted?.pid ?? "none"} engine=${engine?.pid ?? "none"}`,
    );
    await closeHost(host);
    await terminate(
      restarted,
      path.join(cargoTarget(), "debug", "kepler-backend.exe"),
      dataDir,
      "restarted Engine",
    );
    await terminate(
      engine,
      path.join(cargoTarget(), "debug", "kepler-backend.exe"),
      dataDir,
      "Engine",
    );
    for (const pid of teardownPids) await waitForPidGone(pid, `recorded teardown process`);
    recordCleanup(manifestPath, root, teardownPids);
  }
});
