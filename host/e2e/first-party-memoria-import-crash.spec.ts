import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication } from "playwright";
import { test, expect } from "@playwright/test";
import electronBinary from "electron";
import { createSignedApps } from "./fixtures/signed-apps";
import {
  installPartialImportCrashPause,
  readImportCrashDiagnostics,
} from "./fixtures/memoria-partial-import";
import {
  buildEngine,
  cargoTarget,
  closeHost,
  crashProcessTree,
  executableName,
  hostE2eEnvironment,
  processTreePids,
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
const packedHost = process.env.KOSMOS_PACKED_HOST;

test("signed Memoria rolls back an import interrupted after a durable entry write", async () => {
  test.setTimeout(180_000);
  test.skip(!fs.existsSync(hostMain), `build Host first: ${hostMain}`);
  const cleanupManifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!cleanupManifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-memoria-crash-"));
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
  if (process.env.KEPLER_BACKEND_EXE) {
    environment.KEPLER_BACKEND_EXE = process.env.KEPLER_BACKEND_EXE;
  }
  recordCleanup(cleanupManifest, root, new Set());
  let host: ElectronApplication | undefined;
  let engine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  let restartedEngine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  const pids = new Set<number>();
  const launchHost = () =>
    electron.launch({
      executablePath: packedHost ?? electronBinary,
      args: packedHost
        ? [`--user-data-dir=${userData}`, "--open-app", "com.kosmos.memoria"]
        : [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.memoria"],
      env: environment,
      timeout: 30_000,
    });

  try {
    const apps = createSignedApps(root, workspaceRoot, false, false, false, false, true);
    const binaries = buildEngine(apps.trust);
    const version = apps.versions["com.kosmos.memoria"];
    const archive = apps.archives["com.kosmos.memoria"];
    const started = await startEngine(binaries.engine, binaries.ark, dataDir);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    let lock = started.lock;
    const trust = await rpc(lock, "packages.trust_status");
    expect(trust.ok, rpcError(trust)).toBe(true);
    const catalog = await rpc(lock, "packages.catalog_apply", {
      document: apps.catalog,
      signatures: apps.signatures,
    });
    expect(catalog.ok, rpcError(catalog)).toBe(true);
    const install = await rpc(lock, "packages.install", {
      id: "com.kosmos.memoria",
      version,
      archive_path: archive,
    });
    expect(install.ok, rpcError(install)).toBe(true);
    const enable = await rpc(lock, "packages.set_enabled", {
      id: "com.kosmos.memoria",
      version,
      enabled: true,
    });
    expect(enable.ok, rpcError(enable)).toBe(true);

    host = await launchHost();
    pids.add(host.process().pid);
    let page = await host.firstWindow();
    await expect
      .poll(() => page.evaluate(() => Boolean(window.api)), { timeout: 30_000 })
      .toBe(true);
    await page.evaluate(() => {
      window.location.hash = "#/settings";
    });
    await expect(page.getByTestId("eden-settings-export")).toBeVisible();
    await page.getByTestId("eden-settings-export").click();
    await expect(page.getByTestId("eden-import-obsidian-vault")).toBeVisible();
    const baselineEntryIds = await page.evaluate(async () =>
      (await window.api.listAllEntries())
        .map((entry) => entry.id)
        .filter((id) => !id.startsWith("collection:"))
        .sort(),
    );
    await installPartialImportCrashPause(page, path.join(root, "crash-import-vault"));
    await page.getByTestId("eden-import-obsidian-vault").getByRole("button").click();
    await expect
      .poll(() =>
        page.evaluate(() => {
          // SAFETY: the fixture installs this test-only marker before the import starts.
          return Boolean(
            (window as typeof window & { __memoriaPartialImportOpened?: boolean })
              .__memoriaPartialImportOpened,
          );
        }),
      )
      .toBe(true);
    try {
      await expect
        .poll(
          () =>
            page.evaluate(() => {
              // SAFETY: the fixture installs this test-only marker before the import starts.
              return Boolean(
                (window as typeof window & { __memoriaImportFirstWrite?: boolean })
                  .__memoriaImportFirstWrite,
              );
            }),
          { timeout: 30_000 },
        )
        .toBe(true);
    } catch (error) {
      const diagnostics = await readImportCrashDiagnostics(page);
      console.log(
        `[host-e2e] import diagnostics on first-write timeout=${JSON.stringify(diagnostics)}`,
      );
      console.log(
        `[host-e2e] host windows=${JSON.stringify((host?.windows() ?? []).map((win) => win.url()))}`,
      );
      throw error;
    }

    console.log(
      `[host-e2e] journal before crash=${JSON.stringify(await readImportCrashDiagnostics(page))}`,
    );
    await closeHost(host, pids);
    host = undefined;
    const enginePid = engine?.pid;
    if (!enginePid) throw new Error("Memoria crash-boundary Engine PID is unavailable");
    for (const pid of processTreePids(enginePid)) pids.add(pid);
    expect(await crashProcessTree(engine, "Memoria import durable-step crash")).toContain(
      enginePid,
    );
    pids.clear();
    engine = undefined;

    const restarted = await startEngine(binaries.engine, binaries.ark, dataDir);
    restartedEngine = restarted.child;
    if (restartedEngine.pid) pids.add(restartedEngine.pid);
    lock = restarted.lock;
    host = await launchHost();
    pids.add(host.process().pid);
    page = await host.firstWindow();
    await expect
      .poll(() => page.evaluate(() => Boolean(window.api)), { timeout: 30_000 })
      .toBe(true);
    await expect
      .poll(() =>
        page.evaluate(async () =>
          (await window.api.listAllEntries())
            .map((entry) => entry.id)
            .filter((id) => !id.startsWith("collection:"))
            .sort(),
        ),
      )
      .toEqual(baselineEntryIds);
    // Recovery persists one checkpoint per compensated operation, so the
    // journal still reads "applying" while later operations roll back —
    // entry IDs reach baseline as soon as the in-flight write is undone.
    // Wait for the terminal checkpoint, which flips status last.
    let afterRestart: Awaited<
      ReturnType<typeof readImportCrashDiagnostics>
    > | null = null;
    try {
      await expect
        .poll(
          async () => {
            afterRestart = await readImportCrashDiagnostics(page);
            const userData = afterRestart.journal.userData;
            return userData && typeof userData === "object"
              ? ((userData as { status?: unknown }).status ?? null)
              : null;
          },
          { timeout: 30_000 },
        )
        .toBe("rolled-back");
    } finally {
      console.log(
        `[host-e2e] journal after restart=${JSON.stringify(afterRestart)}`,
      );
    }
    expect(
      afterRestart?.journal.userData,
      JSON.stringify(afterRestart?.journal),
    ).toMatchObject({
      status: "rolled-back",
      applied: [],
      inFlight: null,
      rolledBack: expect.arrayContaining([0, 1]),
    });
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
    for (const pid of pids) await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    try {
      recordCleanup(cleanupManifest, root, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "Memoria crash E2E cleanup failed").toHaveLength(0);
  }
});
