import fs from "node:fs";
import path from "node:path";
import type { ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import type { ElectronApplication } from "playwright";
import { createSignedApps } from "../../host/e2e/fixtures/signed-apps";
import { seedStoreCatalog, storeExternalListing, storePackageListing } from "./store-fixture";
import {
  cleanupManifest,
  closeHost,
  engineBinaries,
  launchManager,
  managerE2eRoot,
  managerEnvironment,
  managerMain,
  recordCleanup,
  rpc,
  rpcError,
  startEngine,
  terminate,
  waitForPidGone,
} from "./manager-runtime";
import type { JsonValue } from "./manager-runtime";

const managerRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workspaceRoot = path.resolve(managerRoot, "..");

const isJsonObject = (
  value: JsonValue | undefined,
): value is { readonly [key: string]: JsonValue } =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const isString = (value: JsonValue | undefined): value is string => typeof value === "string";

// Store/catalog smoke on the Manager harness: the Engine boots with a
// persisted test-signed Store Catalog (`store/catalog.json`), the package
// index catalog is applied through `packages.catalog_apply`, and the Manager
// exercises browse/install-metadata surfaces. Outbound HTTPS is pinned to a
// dead local proxy so refresh/install-from-URL fail closed deterministically.
test("Manager Store browses the signed catalog and installs metadata", async () => {
  test.skip(!fs.existsSync(managerMain), `build Manager first: ${managerMain}`);

  const runRoot = managerE2eRoot("store-catalog");
  const dataDir = path.join(runRoot, "data");
  const binaries = engineBinaries();
  const pids = new Set<number>();
  const managerEnv = managerEnvironment(runRoot, dataDir);
  const engineEnv = {
    KOSMOS_TEST_MODE: "1",
    RUST_LOG: "error",
    HTTP_PROXY: "http://127.0.0.1:1/",
    HTTPS_PROXY: "http://127.0.0.1:1/",
    ALL_PROXY: "http://127.0.0.1:1/",
  };
  const apps = createSignedApps(path.join(runRoot, "packages"), workspaceRoot);
  const listing = storePackageListing(
    "store.fixture-app",
    "Fixture Store App",
    "host-e2e-app-a",
    "1.0.0",
  );
  // Mixed-platform fixture: the Windows-only entry must follow the Engine's
  // host platform token — visible on Windows, filtered out elsewhere.
  const windowsOnly = storePackageListing(
    "store.fixture-windows-only",
    "Windows Only App",
    "host-e2e-app-b",
    "1.0.0",
    ["windows"],
  );
  seedStoreCatalog(runRoot, dataDir, 7, "2026-09-01T00:00:00Z", "2030-01-01T00:00:00Z", [
    listing,
    windowsOnly,
    storeExternalListing("external.fixture", "Fixture External", "https://example.com/"),
  ]);
  let engine: ChildProcess | undefined;
  let manager: ElectronApplication | undefined;
  try {
    const started = await startEngine(binaries.engine, binaries.ark, dataDir, engineEnv);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    const lock = started.lock;

    // Engine-level contract: applied package index + seeded Store catalog.
    const applied = await rpc(lock, "packages.catalog_apply", {
      document: apps.catalog,
      signatures: apps.signatures,
    });
    expect(applied.ok, rpcError(applied)).toBe(true);
    const engineCatalog = await rpc(lock, "store.catalog");
    expect(engineCatalog).toMatchObject({ ok: true, data: { state: "fresh", sequence: 7 } });
    // The Engine reports the host OS as a catalog `platforms` token; the
    // Manager filters listings by it instead of branching on the OS itself.
    const platformToken = isJsonObject(engineCatalog.data)
      ? engineCatalog.data.platform
      : undefined;
    const hostPlatform = isString(platformToken) ? platformToken : undefined;
    expect(hostPlatform).toBeTruthy();
    const resolvedUrl = await rpc(lock, "store.external_url", {
      listing_id: "external.fixture",
    });
    expect(resolvedUrl).toMatchObject({ ok: true, data: { url: "https://example.com/" } });

    manager = await launchManager(runRoot, "store", managerEnv);
    pids.add(manager.process().pid);
    const page = await manager.firstWindow();
    await expect(page.locator("[aria-label='Разделы менеджера']")).toBeVisible();

    const snapshot = await page.evaluate(() => window.kosmosManager.getStoreCatalog());
    expect(snapshot).toMatchObject({
      ok: true,
      data: { state: "fresh", sequence: 7, platform: hostPlatform },
    });
    const listingIds = (snapshot.ok ? snapshot.data.listings : []).map((item) => item.id);
    expect(listingIds).toEqual(
      expect.arrayContaining([
        "store.fixture-app",
        "store.fixture-windows-only",
        "external.fixture",
      ]),
    );
    const packages = await page.evaluate(() => window.kosmosManager.getPackages());
    const catalogIds = (packages.ok ? packages.data.catalog : []).map((item) => item.id);
    expect(catalogIds).toEqual(expect.arrayContaining(["host-e2e-app-a", "host-e2e-app-b"]));
    const trust = await page.evaluate(() => window.kosmosManager.getPackageTrustStatus());
    expect(trust).toMatchObject({
      ok: true,
      data: { configured: true, state: "usable", catalog_sequence: 1 },
    });

    // Browse: the signed package listing renders under Маркетплейс — in the
    // main grid and in the "Для ваших данных" recommendations row (its
    // data_compatibility matches the Engine data summary). The external-app
    // listing is intentionally not in tabs.
    const sidebar = page.locator("[aria-label='Разделы менеджера']");
    await sidebar.getByRole("button", { name: "Маркетплейс", exact: true }).click();
    const grid = page.locator("section.store-view > .store-grid");
    const card = grid.locator(".store-card", { hasText: "Fixture Store App" });
    await expect(card).toBeVisible();
    await expect(card.getByRole("button", { name: "Установить" })).toBeVisible();
    await expect(
      page.locator("[aria-label='Для ваших данных'] .store-card", {
        hasText: "Fixture Store App",
      }),
    ).toBeVisible();
    expect(await page.locator(".store-card", { hasText: "Fixture External" }).count()).toBe(0);
    // The grid filters by the Engine-reported host platform: the Windows-only
    // listing renders only when the token is "windows".
    const windowsOnlyCard = page.locator(".store-card", { hasText: "Windows Only App" });
    if (hostPlatform === "windows") await expect(windowsOnlyCard).toBeVisible();
    else await expect(windowsOnlyCard).toHaveCount(0);

    // The same token gates the Интеграции catalog surface: the first-party
    // Huawei Health entry is Windows-only by manifest, so its card follows
    // the host platform rather than always rendering.
    await sidebar.getByRole("button", { name: "Интеграции", exact: true }).click();
    const huaweiCard = page.locator("[data-testid='connection-card-com.kosmos.huawei-health']");
    await expect(huaweiCard).toHaveCount(hostPlatform === "windows" ? 1 : 0);
    await sidebar.getByRole("button", { name: "Маркетплейс", exact: true }).click();
    await expect(card).toBeVisible();

    await card.click();
    const detail = page.locator(".store-detail-app-header");
    await expect(detail.getByRole("heading", { name: "Fixture Store App" })).toBeVisible();
    await expect(page.locator(".store-detail-metadata")).toContainText("1.0.0");

    // UI install goes through manager.installPackage → packages.install →
    // install_from_catalog, which fails closed: the fixture catalog's
    // archive_url (https://fixture.invalid/…) is unreachable through the dead
    // proxy, and the card surfaces the Engine error verbatim.
    await detail.getByRole("button", { name: "Установить" }).click();
    await page.getByLabel("Назад к Marketplace").click();
    await expect(card.locator(".store-card-feedback")).toContainText(
      "packages.install: invalid-request",
    );
    const offlineInstall = await page.evaluate(() =>
      window.kosmosManager.installPackage({ package_id: "host-e2e-app-a", version: "1.0.0" }),
    );
    expect(offlineInstall).toMatchObject({ ok: false, code: "engine" });

    // The working local path: install the signed archive the catalog indexes.
    const install = await rpc(lock, "packages.install", {
      id: "host-e2e-app-a",
      version: "1.0.0",
      archive_path: apps.archives["host-e2e-app-a"],
    });
    expect(install.ok, rpcError(install)).toBe(true);
    const enabled = await rpc(lock, "packages.set_enabled", {
      id: "host-e2e-app-a",
      version: "1.0.0",
      enabled: true,
    });
    expect(enabled.ok, rpcError(enabled)).toBe(true);

    // Install metadata now projects into the Store catalog installed list.
    const afterInstall = await page.evaluate(() => window.kosmosManager.getStoreCatalog());
    const installed = (afterInstall.ok ? afterInstall.data.installed : []).find(
      (item) => item.id === "host-e2e-app-a",
    );
    expect(installed).toMatchObject({
      version: "1.0.0",
      kind: "app",
      enabled: true,
      revoked: false,
    });
    expect(installed?.effective_grants?.some((grant) => grant.type === "com.kosmos.note")).toBe(
      true,
    );
    // Opening a hosted app stays a Host capability: absent under headless.
    const opened = await page.evaluate(() =>
      window.kosmosManager.openPackage({ package_id: "host-e2e-app-a" }),
    );
    expect(opened).toMatchObject({ ok: false, code: "engine" });

    await page.reload();
    await sidebar.getByRole("button", { name: "Маркетплейс", exact: true }).click();
    await expect(card).toBeVisible();
    await expect(card.getByRole("button", { name: "Открыть" })).toBeVisible();
    await page.getByRole("button", { name: "Установленные", exact: true }).click();
    await expect(card).toBeVisible();

    // Refresh stays fail-closed: the test-signed Engine cannot verify the
    // production catalog and the dead proxy guarantees no network path.
    const refreshed = await page.evaluate(() => window.kosmosManager.refreshStoreCatalog());
    expect(refreshed).toMatchObject({ ok: false, code: "engine" });
    const stillFresh = await page.evaluate(() => window.kosmosManager.getStoreCatalog());
    expect(stillFresh).toMatchObject({ ok: true, data: { state: "fresh", sequence: 7 } });

    // store.external_url resolution contract through the Manager boundary.
    const external = await page.evaluate(() =>
      window.kosmosManager.openStoreExternal({ listing_id: "external.fixture" }),
    );
    expect(external).toEqual({ ok: true, data: { opened: false } });
    const notExternal = await page.evaluate(() =>
      window.kosmosManager.openStoreExternal({ listing_id: "store.fixture-app" }),
    );
    expect(notExternal).toMatchObject({ ok: false, code: "engine" });
    const invalid = await page.evaluate(() => window.kosmosManager.openStoreExternal({} as never));
    expect(invalid).toMatchObject({ ok: false, code: "validation" });

    expect(await page.locator("body").innerText()).not.toContain(lock.auth_token);
    const windows = await manager.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows().map((window) => window.isVisible()),
    );
    expect(windows).toEqual([false]);
  } finally {
    const cleanupErrors: unknown[] = [];
    const attempt = async (action: () => Promise<void>) => {
      try {
        await action();
      } catch (error) {
        cleanupErrors.push(error);
      }
    };
    await attempt(() => closeHost(manager, pids));
    await attempt(() => terminate(engine, binaries.engine, dataDir, "Engine"));
    for (const pid of pids) {
      await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    }
    try {
      recordCleanup(cleanupManifest(), runRoot, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "store catalog E2E cleanup failed").toHaveLength(0);
  }
});
