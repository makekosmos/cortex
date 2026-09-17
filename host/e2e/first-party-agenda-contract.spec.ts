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
  executableName,
  hostE2eEnvironment,
  recordCleanup,
  rpc,
  rpcError,
  startEngine,
  terminate,
  waitForPidGone,
} from "./fixtures/host-runtime";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workspaceRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");
test("signed Agenda installs, runs in Host, and survives Engine restart", async () => {
  test.setTimeout(180_000);
  test.skip(!fs.existsSync(hostMain), `build Host first: ${hostMain}`);

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-agenda-"));
  const dataDir = path.join(root, "engine");
  const userData = path.join(root, "host-user-data");
  const environment = hostE2eEnvironment({
    APPDATA: path.join(root, "appdata"),
    XDG_CONFIG_HOME: path.join(root, "xdg-config"),
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_HEADLESS: "1",
    KOSMOS_TEST_MODE: "1",
  });
  // Linux containers typically lack unprivileged user namespaces for the
  // Chromium SUID sandbox.
  if (process.platform === "linux") environment.ELECTRON_DISABLE_SANDBOX = "1";
  const cleanupManifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!cleanupManifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");
  recordCleanup(cleanupManifest, root, new Set());

  let host: ElectronApplication | undefined;
  let engine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  let restartedEngine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  const pids = new Set<number>();
  const launchHost = () =>
    electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.agenda"],
      env: environment,
      timeout: 30_000,
    });

  try {
    const apps = createSignedApps(root, workspaceRoot, false, false, false, true);
    const binaries = buildEngine(apps.trust);
    const version = apps.versions["com.kosmos.agenda"] ?? "0.2.4";
    const archive = apps.archives["com.kosmos.agenda"];
    expect(archive).toBeTruthy();

    const started = await startEngine(binaries.engine, binaries.ark, dataDir);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    let lock = started.lock;
    const initialEnginePid = lock.pid;

    const trust = await rpc(lock, "packages.trust_status");
    expect(trust.ok, rpcError(trust)).toBe(true);
    const catalog = await rpc(lock, "packages.catalog_apply", {
      document: apps.catalog,
      signatures: apps.signatures,
    });
    expect(catalog.ok, rpcError(catalog)).toBe(true);
    const install = await rpc(lock, "packages.install", {
      id: "com.kosmos.agenda",
      version,
      archive_path: archive,
    });
    expect(install.ok, rpcError(install)).toBe(true);
    const enable = await rpc(lock, "packages.set_enabled", {
      id: "com.kosmos.agenda",
      version,
      enabled: true,
    });
    expect(enable.ok, rpcError(enable)).toBe(true);

    host = await launchHost();
    pids.add(host.process().pid);
    const initialHostPid = host.process().pid;
    const page = await host.firstWindow();
    await expect.poll(() => host?.windows().length ?? 0).toBe(1);
    expect(await page.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.agenda",
      version,
    });
    expect(await page.evaluate(() => Object.keys(window.kosmosApp.userData))).toEqual(
      expect.arrayContaining(["read", "write", "delete", "stat"]),
    );

    const result = await page.evaluate(async () => {
      const object = {
        id: "agenda-host-e2e-task",
        typeId: "com.kosmos.task",
        typeVersion: "1.0.0",
        title: "Agenda Host E2E",
        contentJson: { type: "doc", content: [] },
        propsJson: {
          status: "todo",
          priority: "none",
          scheduledAt: null,
          dueAt: null,
          reminderAt: null,
          completedAt: null,
          canceledAt: null,
          recurrence: null,
          checklist: [],
          extensions: {},
        },
        createdAt: "2026-07-01T00:00:00.000Z",
        updatedAt: "2026-07-01T00:00:00.000Z",
        deletedAt: null,
      };
      const references = {
        id: "agenda:references",
        typeId: "com.kosmos.agenda.references",
        typeVersion: "1.0.0",
        title: "Agenda reference entities",
        propsJson: {
          model_version: 1,
          projects: [
            {
              id: "agenda-e2e-project",
              title: "Agenda E2E project",
              status: 0,
              sortOrder: 1,
              createdAt: object.createdAt,
              billable: false,
            },
          ],
          areas: [{ id: "agenda-e2e-area", title: "Area", sortOrder: 1, isVisible: true }],
          tags: [{ id: "agenda-e2e-tag", title: "Tag", color: "#ffffff" }],
          headings: [
            {
              id: "agenda-e2e-heading",
              title: "Agenda E2E heading",
              sortOrder: 1,
              projectId: "agenda-e2e-project",
            },
          ],
          extensions: { source_app: "agenda" },
        },
        createdAt: object.createdAt,
        updatedAt: object.updatedAt,
        deletedAt: null,
      };
      return {
        upsert: await window.kosmosApp.ark.request("upsert_object", { object }),
        get: await window.kosmosApp.ark.request("get_object", { id: object.id }),
        references: await window.kosmosApp.ark.request("upsert_object", { object: references }),
        deniedDelete: await window.kosmosApp.ark.request("delete_object", { id: references.id }),
        afterDenied: await window.kosmosApp.ark.request("get_object", { id: references.id }),
        referenceObject: references,
        denied: await window.kosmosApp.ark.request("upsert_object_type", {
          object_type: { id: "agenda-host-e2e-type", name: "Denied" },
        }),
      };
    });
    expect(result.upsert, JSON.stringify(result.upsert)).toMatchObject({ ok: true });
    expect(result.get, JSON.stringify(result.get)).toMatchObject({
      ok: true,
      data: expect.objectContaining({ id: "agenda-host-e2e-task" }),
    });
    expect(result.references, JSON.stringify(result.references)).toMatchObject({ ok: true });
    expect(result.deniedDelete, JSON.stringify(result.deniedDelete)).toEqual({
      ok: false,
      message: "Engine отклонил операцию: forbidden.",
    });
    expect(result.afterDenied).toMatchObject({
      ok: true,
      data: {
        ...result.referenceObject,
        createdAt: expect.any(String),
        updatedAt: expect.any(String),
      },
    });
    expect(result.denied, JSON.stringify(result.denied)).toEqual({
      ok: false,
      message: "Engine отклонил операцию: invalid-request.",
    });

    const staleSnapshot = await rpc(lock, "get_object_write_snapshot", {
      id: "agenda-host-e2e-task",
    });
    expect(staleSnapshot.ok, rpcError(staleSnapshot)).toBe(true);
    const competingObject = {
      ...result.get.data,
      title: "Agenda competing writer",
    };
    const competingWrite = await rpc(lock, "upsert_object", { object: competingObject });
    expect(competingWrite.ok, rpcError(competingWrite)).toBe(true);
    const staleWrite = await rpc(lock, "upsert_object", {
      object: { ...competingObject, title: "Agenda stale writer" },
      expectedSnapshot: staleSnapshot.data,
    });
    expect(staleWrite.ok).toBe(false);
    expect(rpcError(staleWrite)).toContain("object_conflict:stale_snapshot");

    const updated = await page.evaluate(async (callerSnapshot) => {
      const current = await window.kosmosApp.ark.request("get_object", {
        id: "agenda-host-e2e-task",
      });
      return window.kosmosApp.ark.request("upsert_object", {
        object: { ...current.data, title: "Agenda app update" },
        // The Host must replace app-supplied guards with its trusted current snapshot.
        expectedSnapshot: callerSnapshot,
      });
    }, staleSnapshot.data);
    expect(updated, JSON.stringify(updated)).toMatchObject({ ok: true });

    await closeHost(host, pids);
    host = undefined;
    await terminate(engine, binaries.engine, dataDir, "initial Engine");
    engine = undefined;

    const restarted = await startEngine(binaries.engine, binaries.ark, dataDir);
    restartedEngine = restarted.child;
    if (restartedEngine.pid) pids.add(restartedEngine.pid);
    lock = restarted.lock;
    expect(lock.pid).not.toBe(initialEnginePid);
    const packages = await rpc(lock, "packages.list", { kind: "app" });
    expect(packages.ok, rpcError(packages)).toBe(true);
    expect(packages.data.packages).toContainEqual(
      expect.objectContaining({ id: "com.kosmos.agenda", version, enabled: true }),
    );

    host = await launchHost();
    pids.add(host.process().pid);
    expect(host.process().pid).not.toBe(initialHostPid);
    const restartedPage = await host.firstWindow();
    await expect.poll(() => host?.windows().length ?? 0).toBe(1);
    expect(await restartedPage.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.agenda",
      version,
    });
    const persisted = await restartedPage.evaluate(() =>
      window.kosmosApp.ark.request("get_object", { id: "agenda-host-e2e-task" }),
    );
    expect(persisted).toMatchObject({
      ok: true,
      data: expect.objectContaining({ id: "agenda-host-e2e-task", title: "Agenda app update" }),
    });
    const persistedReferences = await restartedPage.evaluate(() =>
      window.kosmosApp.ark.request("get_object", { id: "agenda:references" }),
    );
    expect(persistedReferences).toEqual({ ok: true, data: result.afterDenied.data });
    const deleted = await restartedPage.evaluate(async () => {
      const remove = await window.kosmosApp.ark.request("delete_object", {
        id: "agenda-host-e2e-task",
      });
      const get = await window.kosmosApp.ark.request("get_object", {
        id: "agenda-host-e2e-task",
      });
      return { remove, get };
    });
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
    await attempt(() => closeHost(host, pids));
    const engineBinary = path.join(cargoTarget(), "debug", executableName("kepler-backend"));
    await attempt(() => terminate(restartedEngine, engineBinary, dataDir, "restarted Engine"));
    await attempt(() => terminate(engine, engineBinary, dataDir, "Engine"));
    for (const pid of pids) {
      await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    }
    try {
      recordCleanup(cleanupManifest, root, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "Agenda E2E cleanup failed").toHaveLength(0);
  }
});
