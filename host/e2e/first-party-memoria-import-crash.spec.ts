import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication } from "playwright";
import { test, expect } from "@playwright/test";
import electronBinary from "electron";
import { createSignedApps } from "./fixtures/signed-apps";
import { installPartialImportCrashPause } from "./fixtures/memoria-partial-import";
import {
  buildEngine,
  cargoTarget,
  closeHost,
  crashProcessTree,
  hostE2eEnvironment,
  processTreePids,
  recordCleanup,
  rpc,
  startEngine,
  terminate,
  waitForPidGone,
} from "./fixtures/host-runtime";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workspaceRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");

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
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_HEADLESS: "1",
    KOSMOS_TEST_MODE: "1",
  });
  recordCleanup(cleanupManifest, root, new Set());
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
    let page = await host.firstWindow();
    await expect.poll(() => page.evaluate(() => Boolean(window.api))).toBe(true);
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
            (window as typeof window & { __memoriaImportFirstWrite?: boolean })
              .__memoriaImportFirstWrite,
          );
        }),
      )
      .toBe(true);

    console.log(
      `[host-e2e] journal before crash=${await page.evaluate(() => localStorage.getItem("memoria.obsidian-import-journal.v1"))}`,
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
    await expect.poll(() => page.evaluate(() => Boolean(window.api))).toBe(true);
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
    expect(cleanupErrors, "Memoria crash E2E cleanup failed").toHaveLength(0);
  }
});
