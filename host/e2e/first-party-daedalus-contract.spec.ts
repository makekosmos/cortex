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

test("signed Daedalus enforces its agents contract in Host", async () => {
  test.setTimeout(180_000);
  if (!fs.existsSync(hostMain)) throw new Error(`build Host first: ${hostMain}`);
  const cleanupManifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!cleanupManifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-daedalus-"));
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
  const pids = new Set<number>();

  try {
    const apps = createSignedApps(root, workspaceRoot, false, false, false, false, false, true);
    expect(apps.versions["com.kosmos.daedalus"]).toBe("0.1.0");
    const archive = apps.archives["com.kosmos.daedalus"];
    expect(archive).toBeTruthy();

    // SAFETY: createSignedApps produces the catalog shape asserted immediately below.
    const catalog = JSON.parse(apps.catalog) as {
      packages: Array<{ manifest: { id: string; version: string; entrypoint: string } }>;
    };
    expect(catalog.packages).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          manifest: expect.objectContaining({
            id: "com.kosmos.daedalus",
            version: "0.1.0",
            entrypoint: "dist/index.html",
          }),
        }),
      ]),
    );

    const binaries = buildEngine(apps.trust);
    const started = await startEngine(binaries.engine, binaries.ark, dataDir);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    const lock = started.lock;

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
          id: "com.kosmos.daedalus",
          version: "0.1.0",
          archive_path: archive,
        })
      ).ok,
    ).toBe(true);
    expect(
      (
        await rpc(lock, "packages.set_enabled", {
          id: "com.kosmos.daedalus",
          version: "0.1.0",
          enabled: true,
        })
      ).ok,
    ).toBe(true);

    host = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.daedalus"],
      env: environment,
      timeout: 30_000,
    });
    pids.add(host.process().pid);
    const page = await host.firstWindow();
    await expect.poll(() => host?.windows().length ?? 0).toBe(1);
    expect(await page.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.daedalus",
      version: "0.1.0",
    });

    const projectPath = path.join(workspaceRoot, "cortex");
    const result = await page.evaluate(async (projectPath) => {
      const read = await window.kosmosApp.ark.request("agents.projects.list", {
        include_archived: true,
      });
      const added = await window.kosmosApp.ark.request("agents.projects.add", {
        path: projectPath,
      });
      const listed = await window.kosmosApp.ark.request("agents.projects.list", {
        include_archived: true,
      });
      const undeclared = await window.kosmosApp.ark.request("agents.projects.list_all", {});
      return { read, added, listed, undeclared };
    }, projectPath);

    expect(result.read, JSON.stringify(result.read)).toMatchObject({ ok: true });
    expect(result.added, JSON.stringify(result.added)).toMatchObject({
      ok: true,
      data: expect.objectContaining({ path: expect.stringContaining(projectPath) }),
    });
    expect(result.listed, JSON.stringify(result.listed)).toMatchObject({
      ok: true,
      data: expect.arrayContaining([
        expect.objectContaining({ path: expect.stringContaining(projectPath) }),
      ]),
    });
    expect(result.undeclared, JSON.stringify(result.undeclared)).toMatchObject({ ok: false });
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
      terminate(engine, path.join(cargoTarget(), "debug", "kepler-backend.exe"), dataDir, "Engine"),
    );
    for (const pid of pids) await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    try {
      recordCleanup(cleanupManifest, root, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "Daedalus E2E cleanup failed").toHaveLength(0);
  }
});
