import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";
import { createSignedApps } from "./fixtures/signed-apps";
import {
  buildEngine,
  cargoTarget,
  closeHost,
  hostE2eEnvironment,
  recordCleanup,
  rpc,
  startEngine,
  terminate,
  waitForPidGone,
} from "./fixtures/host-runtime";
import {
  createGitProject,
  fakeAppServerEnvironment,
  runSessionLifecycle,
} from "./fixtures/daedalus-contract";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workspaceRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");
const repositoryPath = path.join(workspaceRoot, "cortex");
const fakeAppServer = path.join(
  repositoryPath,
  "runtime",
  "tests",
  "fixtures",
  "daedalus-fake-app-server.mjs",
);
const canonicalPath = (value: string) => fs.realpathSync.native(value).toLowerCase();

test("signed Daedalus enforces its agents contract in Host", async () => {
  test.setTimeout(180_000);
  if (!fs.existsSync(hostMain)) throw new Error(`build Host first: ${hostMain}`);
  const cleanupManifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!cleanupManifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-daedalus-"));
  recordCleanup(cleanupManifest, root, new Set());
  const projectPath = createGitProject(root);
  const dataDir = path.join(root, "engine");
  const userData = path.join(root, "host-user-data");
  const environment = hostE2eEnvironment({
    APPDATA: path.join(root, "appdata"),
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_HEADLESS: "1",
    KOSMOS_TEST_MODE: "1",
  });
  const appServerPidFile = path.join(root, "daedalus-app-server-pids.jsonl");
  const appServerEnvironment = fakeAppServerEnvironment(fakeAppServer, appServerPidFile);
  let host: ElectronApplication | undefined;
  let engine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  let restartedEngine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
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
    const started = await startEngine(binaries.engine, binaries.ark, dataDir, appServerEnvironment);
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

    const result = await page.evaluate(async (projectPath) => {
      const read = await window.kosmosApp.ark.request("agents.projects.list", {
        include_archived: true,
      });
      const added = await window.kosmosApp.ark.request("agents.projects.add", {
        path: projectPath,
      });
      const projectId = added.data?.id;
      if (!added.ok || projectId?.constructor !== String || !projectId) {
        throw new Error(`project add returned no id: ${JSON.stringify(added)}`);
      }
      const listed = await window.kosmosApp.ark.request("agents.projects.list", {
        include_archived: true,
      });
      const undeclared = await window.kosmosApp.ark.request("agents.projects.list_all", {});
      const beforeSessions = await window.kosmosApp.ark.request("agents.sessions.list", {
        include_archived: true,
      });
      const forgedFullAccess = await window.kosmosApp.ark.request("agents.sessions.create", {
        project_id: projectId,
        prompt: "forged full-access E2E",
        mode: "full-access",
        full_access_confirmed: true,
        package_id: "com.kosmos.daedalus",
        package_version: "0.1.0",
        model: "gpt-test",
      });
      const afterSessions = await window.kosmosApp.ark.request("agents.sessions.list", {
        include_archived: true,
      });
      const live = await window.kosmosApp.ark.request("agents.projects.list", {
        include_archived: true,
      });
      return {
        read,
        added,
        listed,
        undeclared,
        beforeSessions,
        forgedFullAccess,
        afterSessions,
        live,
      };
    }, projectPath);

    expect(result.read, JSON.stringify(result.read)).toMatchObject({ ok: true });
    expect(result.added, JSON.stringify(result.added)).toMatchObject({ ok: true });
    expect(canonicalPath(String(result.added.data?.path))).toBe(canonicalPath(projectPath));
    expect(result.listed, JSON.stringify(result.listed)).toMatchObject({ ok: true });
    expect(
      result.listed.data?.some(
        (project) => canonicalPath(project.path) === canonicalPath(projectPath),
      ),
    ).toBe(true);
    expect(result.undeclared, JSON.stringify(result.undeclared)).toEqual({
      ok: false,
      message: "Engine отклонил операцию: invalid-request.",
    });
    expect(result.beforeSessions).toEqual({ ok: true, data: [] });
    expect(result.forgedFullAccess).toEqual({
      ok: false,
      message: "Engine отклонил операцию: unavailable.",
    });
    expect(result.afterSessions).toEqual({ ok: true, data: [] });
    const audit = JSON.parse(
      execFileSync(
        "bun",
        [
          "--eval",
          'import {Database} from "bun:sqlite"; const db=new Database(process.argv[1],{readonly:true}); process.stdout.write(JSON.stringify(db.query("SELECT event,result,package_id,package_version,project_id,mode,model FROM security_audit ORDER BY id DESC LIMIT 1").get()))',
          path.join(dataDir, "extensions-data", "daedalus", "daedalus.db"),
        ],
        { encoding: "utf8", windowsHide: true },
      ),
    );
    expect(audit).toEqual({
      event: "consent_consume",
      result: "denied",
      package_id: "com.kosmos.daedalus",
      package_version: "0.1.0",
      project_id: result.added.data?.id,
      mode: "full-access",
      model: "gpt-test",
    });
    expect(result.live).toMatchObject({ ok: true });
    expect(
      result.live.data?.some(
        (project) => canonicalPath(project.path) === canonicalPath(projectPath),
      ),
    ).toBe(true);

    if (!engine?.pid || result.added.data?.id?.constructor !== String)
      throw new Error("Engine PID or project id is missing");
    await runSessionLifecycle(page, result.added.data.id, appServerPidFile, pids);

    await closeHost(host, pids);
    host = undefined;
    await terminate(engine, binaries.engine, dataDir, "initial Engine");
    engine = undefined;

    const restarted = await startEngine(
      binaries.engine,
      binaries.ark,
      dataDir,
      appServerEnvironment,
    );
    restartedEngine = restarted.child;
    if (restartedEngine.pid) pids.add(restartedEngine.pid);
    host = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.daedalus"],
      env: environment,
      timeout: 30_000,
    });
    pids.add(host.process().pid);
    const restartedPage = await host.firstWindow();
    await expect.poll(() => host?.windows().length ?? 0).toBe(1);
    const restartedProjects = await restartedPage.evaluate(() =>
      window.kosmosApp.ark.request("agents.projects.list", { include_archived: true }),
    );
    expect(restartedProjects).toMatchObject({ ok: true });
    expect(
      restartedProjects.data?.some(
        (project) => canonicalPath(project.path) === canonicalPath(projectPath),
      ),
    ).toBe(true);
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
    expect(cleanupErrors, "Daedalus E2E cleanup failed").toHaveLength(0);
  }
});
