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
  startEngine,
  terminate,
  waitForPidGone,
} from "./fixtures/host-runtime";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workspaceRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");

test("signed Memoria runs CRUD through Host and survives Engine restart", async () => {
  test.setTimeout(180_000);
  test.skip(!fs.existsSync(hostMain), `build Host first: ${hostMain}`);
  const cleanupManifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!cleanupManifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-memoria-"));
  const dataDir = path.join(root, "engine");
  const userData = path.join(root, "host-user-data");
  const environment = {
    ...process.env,
    APPDATA: path.join(root, "appdata"),
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_HEADLESS: "1",
    KOSMOS_TEST_MODE: "1",
  };
  let host: ElectronApplication | undefined;
  let engine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  let restartedEngine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  const pids = new Set<number>();
  const launchHost = () =>
    electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.memoria"],
      env: environment,
      timeout: 30_000,
    });

  try {
    const apps = createSignedApps(root, workspaceRoot, false, false, false, false, true);
    const binaries = buildEngine(apps.trust);
    const version = apps.versions["com.kosmos.memoria"];
    const archive = apps.archives["com.kosmos.memoria"];
    expect(version).toBe("0.6.3");
    expect(archive).toBeTruthy();

    const started = await startEngine(binaries.engine, binaries.ark, dataDir);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    let lock = started.lock;

    expect((await rpc(lock, "packages.trust_status")).ok).toBe(true);
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
          id: "com.kosmos.memoria",
          version,
          archive_path: archive,
        })
      ).ok,
    ).toBe(true);
    expect(
      (
        await rpc(lock, "packages.set_enabled", {
          id: "com.kosmos.memoria",
          version,
          enabled: true,
        })
      ).ok,
    ).toBe(true);

    host = await launchHost();
    pids.add(host.process().pid);
    const initialHostPid = host.process().pid;
    const page = await host.firstWindow();
    await expect.poll(() => host?.windows().length ?? 0).toBe(1);
    expect(await page.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.memoria",
      version,
    });

    const first = await page.evaluate(async () => {
      const object = {
        id: "memoria-host-e2e-note",
        typeId: "com.kosmos.note",
        typeVersion: "1.0.0",
        title: "Memoria Host E2E",
        contentJson: { type: "doc", content: [] },
        propsJson: { description: null, extensions: {} },
        createdAt: "2026-07-01T00:00:00.000Z",
        updatedAt: "2026-07-01T00:00:00.000Z",
        deletedAt: null,
      };
      return {
        create: await window.kosmosApp.ark.request("upsert_object", { object }),
        read: await window.kosmosApp.ark.request("get_object", { id: object.id }),
        denied: await Promise.all([
          window.kosmosApp.ark.request("upsert_object_type", {
            object_type: { id: "memoria-host-e2e-denied", name: "Denied" },
          }),
          window.kosmosApp.ark.request("focus.get_active_state", {}),
          window.kosmosApp.ark.request("dictation.get_config", {}),
          window.kosmosApp.ark.request("upsert_object", {
            object: { ...object, id: "memoria-host-e2e-foreign", typeId: "com.kosmos.secret" },
          }),
        ]),
      };
    });
    expect(first.create, JSON.stringify(first.create)).toMatchObject({ ok: true });
    expect(first.read, JSON.stringify(first.read)).toMatchObject({
      ok: true,
      data: expect.objectContaining({ id: "memoria-host-e2e-note", title: "Memoria Host E2E" }),
    });
    expect(first.denied, JSON.stringify(first.denied)).toHaveLength(4);
    for (const denied of first.denied) expect(denied).toMatchObject({ ok: false });

    const updated = await page.evaluate(async () => ({
      update: await window.kosmosApp.ark.request("upsert_object", {
        object: {
          id: "memoria-host-e2e-note",
          typeId: "com.kosmos.note",
          typeVersion: "1.0.0",
          title: "Memoria Host E2E updated",
          contentJson: { type: "doc", content: [] },
          propsJson: { description: "updated", extensions: {} },
          createdAt: "2026-07-01T00:00:00.000Z",
          updatedAt: "2026-07-01T00:00:01.000Z",
          deletedAt: null,
        },
      }),
      read: await window.kosmosApp.ark.request("get_object", { id: "memoria-host-e2e-note" }),
    }));
    expect(updated.update, JSON.stringify(updated.update)).toMatchObject({ ok: true });
    expect(updated.read, JSON.stringify(updated.read)).toMatchObject({
      ok: true,
      data: expect.objectContaining({ title: "Memoria Host E2E updated" }),
    });

    await closeHost(host);
    host = undefined;
    await terminate(engine, binaries.engine, dataDir, "initial Engine");
    engine = undefined;

    const restarted = await startEngine(binaries.engine, binaries.ark, dataDir);
    restartedEngine = restarted.child;
    if (restartedEngine.pid) pids.add(restartedEngine.pid);
    lock = restarted.lock;
    expect(lock.pid).not.toBe(started.lock.pid);
    expect(await rpc(lock, "packages.list", { kind: "app" })).toMatchObject({
      ok: true,
      data: {
        packages: expect.arrayContaining([
          expect.objectContaining({ id: "com.kosmos.memoria", version, enabled: true }),
        ]),
      },
    });

    host = await launchHost();
    pids.add(host.process().pid);
    expect(host.process().pid).not.toBe(initialHostPid);
    const restartedPage = await host.firstWindow();
    await expect.poll(() => host?.windows().length ?? 0).toBe(1);
    expect(await restartedPage.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.memoria",
      version,
    });
    const persisted = await restartedPage.evaluate(() =>
      window.kosmosApp.ark.request("get_object", { id: "memoria-host-e2e-note" }),
    );
    expect(persisted, JSON.stringify(persisted)).toMatchObject({
      ok: true,
      data: expect.objectContaining({
        id: "memoria-host-e2e-note",
        title: "Memoria Host E2E updated",
      }),
    });

    const deleted = await restartedPage.evaluate(async () => ({
      remove: await window.kosmosApp.ark.request("delete_object", {
        id: "memoria-host-e2e-note",
      }),
      get: await window.kosmosApp.ark.request("get_object", { id: "memoria-host-e2e-note" }),
    }));
    expect(deleted.remove, JSON.stringify(deleted.remove)).toMatchObject({ ok: true });
    expect(deleted.get, JSON.stringify(deleted.get)).toMatchObject({ ok: true, data: null });
  } finally {
    const cleanupErrors: unknown[] = [];
    const attempt = async (action: () => Promise<void>) => {
      try {
        await action();
      } catch (error) {
        cleanupErrors.push(error);
      }
    };
    if (host) pids.add(host.process().pid);
    await attempt(() => closeHost(host));
    await attempt(() =>
      terminate(
        restartedEngine,
        path.join(cargoTarget(), "debug", "kepler-backend.exe"),
        dataDir,
        "restarted Engine",
      ),
    );
    await attempt(() =>
      terminate(engine, path.join(cargoTarget(), "debug", "kepler-backend.exe"), dataDir, "Engine"),
    );
    for (const pid of pids) await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    try {
      recordCleanup(cleanupManifest, root, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "Memoria E2E cleanup failed").toHaveLength(0);
  }
});
