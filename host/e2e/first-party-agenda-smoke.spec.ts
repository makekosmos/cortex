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

// Daily smoke: install signed Agenda on the Engine, open it in the Host, then
// capture a task through the real packaged UI, see it in Входящие (Inbox),
// and open its task card.
test("Agenda capture → Inbox → task card", async () => {
  test.setTimeout(180_000);
  test.skip(!fs.existsSync(hostMain), `build Host first: ${hostMain}`);

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-agenda-smoke-"));
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
  const pids = new Set<number>();

  try {
    const apps = createSignedApps(root, workspaceRoot, false, false, false, true);
    const binaries = buildEngine(apps.trust);
    const version = apps.versions["com.kosmos.agenda"];
    const archive = apps.archives["com.kosmos.agenda"];
    expect(archive).toBeTruthy();

    const started = await startEngine(binaries.engine, binaries.ark, dataDir);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    const lock = started.lock;

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

    host = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.agenda"],
      env: environment,
      timeout: 30_000,
    });
    pids.add(host.process().pid);

    const page = await host.firstWindow();
    await expect.poll(() => host?.windows().length ?? 0).toBe(1);

    // The packaged Agenda UI must hydrate from ARK before capture works.
    const newTaskButton = page.getByRole("button", { name: "Новая задача", exact: true });
    await newTaskButton.waitFor({ state: "visible", timeout: 30_000 });
    await newTaskButton.click();

    const title = "KOS-53 smoke task";
    const titleInput = page.locator("input.quick-entry-panel__title-input");
    await titleInput.waitFor({ state: "visible" });
    await titleInput.fill(title);
    await titleInput.press("Enter");

    // Inbox (Входящие) lists the captured task row.
    const row = page.locator("[data-todo-id]", { hasText: title });
    await row.waitFor({ state: "visible", timeout: 15_000 });

    // Open the task card from the row. Agenda uses memory history, so the
    // card is identified by its title field rather than the URL.
    await row.getByRole("button", { name: "Открыть" }).click();
    await expect(page.getByRole("button", { name: "Назад" })).toBeVisible({ timeout: 15_000 });
    await expect(page.locator("label:has-text('Название') input")).toHaveValue(title, {
      timeout: 15_000,
    });

    // The task is persisted through the Host-scoped ARK bridge.
    const listed = await page.evaluate(async () => {
      const result = await window.kosmosApp.ark.request("list_objects", {
        typeId: "com.kosmos.task",
      });
      return result;
    });
    expect(JSON.stringify(listed)).toContain(title);
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
        engine,
        path.join(cargoTarget(), "debug", executableName("kepler-backend")),
        dataDir,
        "Engine",
      ),
    );
    for (const pid of pids) {
      await attempt(() => waitForPidGone(pid, "recorded teardown process"));
    }
    try {
      recordCleanup(cleanupManifest, root, pids);
    } catch (error) {
      cleanupErrors.push(error);
    }
    expect(cleanupErrors, "Agenda smoke cleanup failed").toHaveLength(0);
  }
});
