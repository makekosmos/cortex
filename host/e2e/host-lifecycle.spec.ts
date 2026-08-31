import { spawn, type ChildProcess } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";
import { createSignedApps } from "./fixtures/signed-apps";
import {
  buildEngine,
  cargoTarget,
  closeHost,
  recordCleanup,
  rpc,
  rpcError,
  startEngine,
  terminate,
  waitForPidGone,
  type JsonValue,
  type Lock,
} from "./fixtures/host-runtime";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");

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
      // SAFETY: packages.trust_status returns the documented trust object.
      (trustStatus.data as { trust?: { configured?: JsonValue } } | undefined)?.trust?.configured,
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
      // SAFETY: the Engine lock file is written by the test Engine and contains its pid.
      JSON.parse(fs.readFileSync(path.join(dataDir, "engine.lock.json"), "utf8")) as Lock,
    ).toMatchObject({ pid: initialEnginePid });
    expect(fs.existsSync(path.join(appData, "Kosmos", "engine.lock.json"))).toBe(false);
    // SAFETY: the Engine info endpoint returns the documented protocol usage shape.
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
        clients: Record<string, JsonValue>;
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
    await closeHost(host, teardownPids);
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
