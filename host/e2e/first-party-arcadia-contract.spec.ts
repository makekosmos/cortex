import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication } from "playwright";
import { test, expect } from "@playwright/test";
import electronBinary from "electron";
import { createSignedApps } from "./fixtures/signed-apps";
import { cleanupArcadiaE2e } from "./fixtures/arcadia-cleanup";
import { ARCADIA_EFFECTIVE_GRANTS, type ArcadiaInstalledPackage } from "./fixtures/arcadia-archive";
import { createArcadiaFixtures, expectHostileSteamRejected } from "./fixtures/arcadia-steam";
import { createSqobaRoot, exerciseSqoba, expectSqobaRecovered } from "./fixtures/arcadia-sqoba";
import {
  buildEngine,
  closeHost,
  crashProcessTree,
  hostE2eEnvironment,
  processTreePids,
  recordCleanup,
  rpc,
  startEngine,
} from "./fixtures/host-runtime";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workspaceRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");
test("signed Arcadia enforces exact grants and recovers after an Engine crash", async () => {
  test.setTimeout(180_000);
  if (!fs.existsSync(hostMain)) throw new Error(`build Host first: ${hostMain}`);
  const cleanupManifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!cleanupManifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-arcadia-"));
  recordCleanup(cleanupManifest, root, new Set());
  const dataDir = path.join(root, "engine");
  const userData = path.join(root, "host-user-data");
  const saveRoot = createSqobaRoot(root);
  const environment = hostE2eEnvironment({
    APPDATA: path.join(root, "appdata"),
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_HEADLESS: "1",
    KOSMOS_TEST_MODE: "1",
    KOSMOS_TEST_SELECTED_DIRECTORY: saveRoot,
  });
  const { fakeExe, hostileSteam } = createArcadiaFixtures(dataDir);
  let host: ElectronApplication | undefined;
  let engine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  let restartedEngine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  let sqobaRecovery: Awaited<ReturnType<typeof exerciseSqoba>> | undefined;
  const pids = new Set<number>();
  const launchHost = () =>
    electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.arcadia"],
      env: environment,
      timeout: 30_000,
    });
  const openHost = async () => {
    const launched = await launchHost();
    host = launched;
    pids.add(launched.process().pid);
    const page = await launched.firstWindow();
    await expect.poll(() => launched.windows().length).toBe(1);
    return page;
  };
  try {
    const apps = createSignedApps(
      root,
      workspaceRoot,
      false,
      false,
      false,
      false,
      false,
      false,
      false,
      false,
      true,
    );
    const version = apps.versions["com.kosmos.arcadia"];
    const archive = apps.archives["com.kosmos.arcadia"];
    expect(version).toBe("0.1.11");
    expect(archive).toBeTruthy();
    const catalog: {
      packages: Array<{ manifest: { id: string; version: string }; sha256: string }>;
    } = JSON.parse(apps.catalog);
    expect(catalog.packages).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          manifest: expect.objectContaining({ id: "com.kosmos.arcadia", version: "0.1.11" }),
          sha256: "a05ae74be8c7bafa86f4ef91d11445ec7e256b9bd27ececd35996b4f5f83be6c",
        }),
      ]),
    );
    const binaries = buildEngine(apps.trust);
    const started = await startEngine(binaries.engine, binaries.ark, dataDir);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    let lock = started.lock;
    expect(await rpc(lock, "packages.list", { kind: "app" })).toMatchObject({
      ok: true,
      data: { packages: [], total: 0 },
    });
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
          id: "com.kosmos.arcadia",
          version,
          archive_path: archive,
        })
      ).ok,
    ).toBe(true);
    expect(
      (
        await rpc(lock, "packages.set_enabled", {
          id: "com.kosmos.arcadia",
          version,
          enabled: true,
        })
      ).ok,
    ).toBe(true);
    expect(await rpc(lock, "packages.list", { kind: "app" })).toMatchObject({
      ok: true,
      data: {
        total: 1,
        packages: [
          expect.objectContaining({
            id: "com.kosmos.arcadia",
            version,
            enabled: true,
            worker_state: "running",
          }),
        ],
      },
    });
    const storeCatalog = await rpc(lock, "store.catalog");
    expect(storeCatalog.ok).toBe(true);
    // SAFETY: store.catalog has a versioned installed-package envelope asserted immediately below.
    const installed = (storeCatalog.data as { installed?: ArcadiaInstalledPackage[] } | undefined)
      ?.installed;
    expect(installed).toHaveLength(1);
    expect(installed?.[0]).toMatchObject({
      id: "com.kosmos.arcadia",
      version,
      enabled: true,
    });
    expect(installed?.[0]?.effective_grants).toEqual(ARCADIA_EFFECTIVE_GRANTS);
    const page = await openHost();
    expect(await page.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.arcadia",
      version,
    });
    const initialGames = await page.evaluate(() => window.kosmosApp.ark!.request("games.list", {}));
    expect(initialGames, JSON.stringify(initialGames)).toMatchObject({ ok: true, data: [] });
    await expectHostileSteamRejected(page, hostileSteam);
    sqobaRecovery = await exerciseSqoba(page, dataDir, fakeExe, saveRoot);
    const first = await page.evaluate(async (exePath) => {
      const added = await window.kosmosApp.ark!.request("games.add_manual", {
        name: "Arcadia Host E2E",
        exe_path: exePath,
        save_roots: [],
      });
      // SAFETY: games.add_manual returns the installed Arcadia worker's documented result envelope.
      const id = (added as { data?: { id?: string } } | null)?.data?.id;
      return {
        id,
        added,
        listed: await window.kosmosApp.ark!.request("games.list", {}),
        read: id
          ? await window.kosmosApp.ark!.request("games.read", { id })
          : { ok: false, message: "missing-game-id" },
        undeclaredType: await window.kosmosApp.ark!.request("upsert_object_type", {
          object_type: { id: "arcadia-host-e2e-undeclared", name: "Denied" },
        }),
        foreignType: await window.kosmosApp.ark!.request("upsert_object", {
          object: {
            id: "arcadia-host-e2e-foreign",
            typeId: "com.kosmos.note",
            typeVersion: "1.0.0",
            title: "Denied",
          },
        }),
      };
    }, fakeExe);
    expect(first.added, JSON.stringify(first.added)).toMatchObject({
      ok: true,
      data: { ok: true, id: expect.any(String) },
    });
    expect(first.id).toEqual(expect.any(String));
    if (!first.id) throw new Error("games.add_manual did not return an id");
    const gameId = first.id;
    expect(first.listed, JSON.stringify(first.listed)).toMatchObject({
      ok: true,
      data: expect.arrayContaining([
        expect.objectContaining({
          id: gameId,
          title: "Arcadia Host E2E",
        }),
      ]),
    });
    expect(first.read, JSON.stringify(first.read)).toMatchObject({
      ok: true,
      data: {
        id: gameId,
        title: "Arcadia Host E2E",
        local: expect.objectContaining({ exePath: fakeExe }),
      },
    });
    expect(first.undeclaredType).toEqual({
      ok: false,
      message: "Engine отклонил операцию: invalid-request.",
    });
    expect(first.foreignType).toEqual({
      ok: false,
      message: "Engine отклонил операцию: forbidden.",
    });
    await page.reload();
    await expect(page.getByText("Arcadia Host E2E").first()).toBeVisible();
    const initialHostPid = host?.process().pid;
    await closeHost(host, pids);
    host = undefined;
    const enginePid = engine?.pid;
    if (!enginePid) throw new Error("initial Engine PID is unavailable");
    for (const pid of processTreePids(enginePid)) pids.add(pid);
    const crashed = await crashProcessTree(engine, "initial Engine crash injection");
    expect(crashed).toContain(enginePid);
    for (const pid of crashed) pids.add(pid);
    // Windows can reuse numeric PIDs; these processes have been reaped before the restart.
    pids.clear();
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
          expect.objectContaining({ id: "com.kosmos.arcadia", version, enabled: true }),
        ]),
      },
    });
    await expect
      .poll(async () => {
        const result = await rpc(lock, "packages.list", { kind: "app" });
        // SAFETY: packages.list is a versioned Engine endpoint and only these fields are consumed.
        const data = result.data as
          | { packages?: Array<{ id?: string; worker_state?: string }> }
          | undefined;
        return data?.packages?.find((item) => item.id === "com.kosmos.arcadia")?.worker_state;
      })
      .toBe("running");
    if (restartedEngine.pid) for (const pid of processTreePids(restartedEngine.pid)) pids.add(pid);

    const restartedPage = await openHost();
    expect(host?.process().pid).not.toBe(initialHostPid);
    expect(await restartedPage.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.arcadia",
      version,
    });
    if (!sqobaRecovery) throw new Error("SQOBA recovery fixture was not prepared");
    await expectSqobaRecovered(restartedPage, sqobaRecovery);
    const persisted = await restartedPage.evaluate(
      async (id) => ({
        listed: await window.kosmosApp.ark!.request("games.list", {}),
        read: await window.kosmosApp.ark!.request("games.read", { id }),
      }),
      gameId,
    );
    expect(persisted.listed, JSON.stringify(persisted.listed)).toMatchObject({
      ok: true,
      data: expect.arrayContaining([
        expect.objectContaining({
          id: gameId,
          title: "Arcadia Host E2E",
          local: expect.objectContaining({ exePath: fakeExe }),
        }),
      ]),
    });
    expect(persisted.read, JSON.stringify(persisted.read)).toMatchObject({
      ok: true,
      data: { id: gameId, local: expect.objectContaining({ exePath: fakeExe }) },
    });
  } finally {
    await cleanupArcadiaE2e({
      host,
      engine,
      restartedEngine,
      dataDir,
      root,
      cleanupManifest,
      pids,
    });
  }
});
