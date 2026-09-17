import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication } from "playwright";
import { test, expect } from "@playwright/test";
import electronBinary from "electron";
import {
  expectOverdueThirdWork,
  initializeOrdo,
  responseMessage,
  responseNumber,
  writeOverdueThirdWorkState,
} from "./fixtures/ordo-runtime";
import { createSignedApps } from "./fixtures/signed-apps";
import {
  buildEngine,
  cargoTarget,
  closeHost,
  executableName,
  hostE2eEnvironment,
  recordCleanup,
  rpc,
  startEngine,
  terminate,
  type JsonValue,
  waitForPidGone,
} from "./fixtures/host-runtime";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workspaceRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");
const assertResponse = <T>(value: JsonValue, expected: T) =>
  expect(value, JSON.stringify(value)).toMatchObject(expected);
const expectOk = (value: JsonValue) => assertResponse(value, { ok: true });
const expectPhase = (value: JsonValue, phase: "idle" | "work", isPaused: boolean) =>
  assertResponse(value, {
    ok: true,
    data: { phase, isRunning: phase !== "idle", isPaused },
  });

test("signed Ordo enforces its v2 contract in Host", async () => {
  test.setTimeout(180_000);
  if (!fs.existsSync(hostMain)) throw new Error(`build Host first: ${hostMain}`);
  const cleanupManifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!cleanupManifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-ordo-"));
  recordCleanup(cleanupManifest, root, new Set());
  const dataDir = path.join(root, "engine");
  const userData = path.join(root, "host-user-data");
  const environment = hostE2eEnvironment({
    APPDATA: path.join(root, "appdata"),
    XDG_CONFIG_HOME: path.join(root, "xdg-config"),
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_HEADLESS: "1",
    KOSMOS_TEST_MODE: "1",
  });
  // Linux containers lack unprivileged user namespaces for the Chromium sandbox.
  if (process.platform === "linux") environment.ELECTRON_DISABLE_SANDBOX = "1";
  let host: ElectronApplication | undefined;
  let engine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  let restartedEngine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  const pids = new Set<number>();
  const openHost = async () => {
    const launched = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.focus"],
      env: environment,
      timeout: 30_000,
    });
    host = launched;
    pids.add(launched.process().pid);
    const page = await launched.firstWindow();
    await expect.poll(() => launched.windows().length).toBe(1);
    return { page, pid: launched.process().pid };
  };

  try {
    const signed = createSignedApps;
    const apps = signed(root, workspaceRoot, false, false, false, false, false, false, false, true);
    const version = apps.versions["com.kosmos.focus"];
    const archive = apps.archives["com.kosmos.focus"];

    const catalog: {
      packages: Array<{
        manifest: { id: string; version: string; entrypoint: string; schema_version: number };
        sha256: string;
      }>;
    } = JSON.parse(apps.catalog);
    expect(catalog.packages).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          manifest: expect.objectContaining({
            schema_version: 2,
            id: "com.kosmos.focus",
            version: "0.1.3",
            entrypoint: "dist/index.html",
          }),
          sha256: "a546cd6085a9092a4ef50dd3db9a1ad8ad6f47069f8c0da03f52735d3d7a6e9a",
        }),
      ]),
    );

    const binaries = buildEngine(apps.trust);
    const started = await startEngine(binaries.engine, binaries.ark, dataDir);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    let lock = started.lock;

    expectOk(await rpc(lock, "packages.trust_status"));
    expectOk(
      await rpc(lock, "packages.catalog_apply", {
        document: apps.catalog,
        signatures: apps.signatures,
      }),
    );
    expectOk(
      await rpc(lock, "packages.install", {
        id: "com.kosmos.focus",
        version,
        archive_path: archive,
      }),
    );
    expectOk(
      await rpc(lock, "packages.set_enabled", { id: "com.kosmos.focus", version, enabled: true }),
    );

    const { page, pid: firstHostPid } = await openHost();
    assertResponse(await page.evaluate(() => window.kosmosApp.identity), {
      id: "com.kosmos.focus",
      version,
    });
    await expect(page.getByText("Ordo").first()).toBeVisible();

    const initial = await initializeOrdo(page);
    assertResponse(initial.custom, {
      ok: true,
      data: { name: "Host E2E custom", domains: ["example.com", "e2e.invalid"] },
    });
    const customId = initial.id;
    assertResponse(initial.listed, {
      ok: true,
      data: { blocklists: expect.arrayContaining([expect.objectContaining({ id: customId })]) },
    });
    expectOk(initial.active);
    assertResponse(initial.state, {
      ok: true,
      data: { active: true, blocklist_id: customId },
    });
    assertResponse(initial.domains, {
      ok: true,
      data: { domains: ["example.com", "e2e.invalid"] },
    });
    expectPhase(initial.start, "work", false);
    expectPhase(initial.paused, "work", true);
    expectPhase(initial.resumed, "work", false);
    for (const denied of initial.denied) {
      expect(responseMessage(denied)).toBe("Engine отклонил операцию: invalid-request.");
    }

    await closeHost(host, pids);
    host = undefined;
    await terminate(
      engine,
      path.join(cargoTarget(), "debug", executableName("kepler-backend")),
      dataDir,
      "initial Engine",
    );
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
          expect.objectContaining({ id: "com.kosmos.focus", version, enabled: true }),
        ]),
      },
    });

    const relaunched = await openHost();
    let restartedPage = relaunched.page;
    expect(relaunched.pid).not.toBe(firstHostPid);
    const runningAfterRestart = await restartedPage.evaluate(async () => ({
      listed: await window.kosmosApp.ark.request("focus.list_blocklists", {}),
      active: await window.kosmosApp.ark.request("focus.get_active_state", {}),
      pomodoro: await window.kosmosApp.ark.request("pomodoro.get_state", {}),
    }));
    assertResponse(runningAfterRestart.listed, {
      ok: true,
      data: { blocklists: expect.arrayContaining([expect.objectContaining({ id: customId })]) },
    });
    assertResponse(runningAfterRestart.active, {
      ok: true,
      data: { active: true, blocklist_id: customId },
    });
    expectPhase(runningAfterRestart.pomodoro, "work", false);
    assertResponse(runningAfterRestart.pomodoro, {
      data: {
        title: "Ordo Host E2E",
        remainingMs: expect.any(Number),
        phaseEndsAtMs: expect.any(Number),
      },
    });
    expect(responseNumber(runningAfterRestart.pomodoro, "phaseEndsAtMs")).toBe(
      responseNumber(initial.resumed, "phaseEndsAtMs"),
    );
    expect(responseNumber(runningAfterRestart.pomodoro, "remainingMs")).toBeLessThanOrEqual(
      responseNumber(initial.resumed, "remainingMs"),
    );

    const paused = await restartedPage.evaluate(() =>
      window.kosmosApp.ark.request("pomodoro.pause", {}),
    );
    expectPhase(paused, "work", true);
    await closeHost(host, pids);
    host = undefined;

    restartedPage = (await openHost()).page;
    const afterHostRestart = await restartedPage.evaluate(async () => ({
      pomodoro: await window.kosmosApp.ark.request("pomodoro.get_state", {}),
      focus: await window.kosmosApp.ark.request("focus.get_active_state", {}),
    }));
    expectPhase(afterHostRestart.pomodoro, "work", true);
    expect(responseNumber(afterHostRestart.pomodoro, "remainingMs")).toBe(
      responseNumber(paused, "remainingMs"),
    );
    assertResponse(afterHostRestart.focus, {
      ok: true,
      data: { active: true, blocklist_id: customId },
    });
    const resumed = await restartedPage.evaluate(() =>
      window.kosmosApp.ark.request("pomodoro.resume", {}),
    );
    expectPhase(resumed, "work", false);

    await closeHost(host, pids);
    host = undefined;
    await terminate(restartedEngine, binaries.engine, dataDir, "second Engine");
    restartedEngine = undefined;
    writeOverdueThirdWorkState(dataDir);
    const overdueRestart = await startEngine(binaries.engine, binaries.ark, dataDir);
    restartedEngine = overdueRestart.child;
    if (restartedEngine.pid) pids.add(restartedEngine.pid);
    restartedPage = (await openHost()).page;
    const overdue = await restartedPage.evaluate(() =>
      window.kosmosApp.ark.request("pomodoro.get_state", {}),
    );
    expectOverdueThirdWork(overdue);

    const cleaned = await restartedPage.evaluate(
      async (id) => ({
        stopped: await window.kosmosApp.ark.request("pomodoro.stop", {}),
        deactivated: await window.kosmosApp.ark.request("focus.set_active_state", {
          active: false,
        }),
        deleted: await window.kosmosApp.ark.request("focus.delete_blocklist", { id }),
        listed: await window.kosmosApp.ark.request("focus.list_blocklists", {}),
        active: await window.kosmosApp.ark.request("focus.get_active_state", {}),
      }),
      customId,
    );
    expectPhase(cleaned.stopped, "idle", false);
    for (const response of [cleaned.deactivated, cleaned.deleted]) expectOk(response);
    assertResponse(cleaned.listed, {
      ok: true,
      data: { blocklists: expect.not.arrayContaining([expect.objectContaining({ id: customId })]) },
    });
    assertResponse(cleaned.active, {
      ok: true,
      data: { active: false },
    });
  } finally {
    const cleanupErrors: unknown[] = [];
    const engineBinary = path.join(cargoTarget(), "debug", executableName("kepler-backend"));
    const attempt = async (action: () => Promise<void>) => {
      try {
        await action();
      } catch (error) {
        cleanupErrors.push(error);
      }
    };
    if (host) pids.add(host.process().pid);
    await attempt(() => closeHost(host, pids));
    await attempt(() => terminate(restartedEngine, engineBinary, dataDir, "restarted Engine"));
    await attempt(() => terminate(engine, engineBinary, dataDir, "Engine"));
    for (const pid of pids) await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    try {
      recordCleanup(cleanupManifest, root, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "Ordo E2E cleanup failed").toHaveLength(0);
  }
});
