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

// Daily smoke: install signed Memoria on the Engine, open it in the Host, then
// create a note through the real packaged UI, type a title and body, and see
// the card in Всё (Everything) — with the object verified through the ARK
// bridge.
test("Memoria create note → edit → persist", async () => {
  test.setTimeout(180_000);
  test.skip(!fs.existsSync(hostMain), `build Host first: ${hostMain}`);

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-memoria-smoke-"));
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
    const apps = createSignedApps(root, workspaceRoot, false, false, false, false, true);
    const binaries = buildEngine(apps.trust);
    const version = apps.versions["com.kosmos.memoria"];
    const archive = apps.archives["com.kosmos.memoria"];
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

    host = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.memoria"],
      env: environment,
      timeout: 30_000,
    });
    pids.add(host.process().pid);

    const page = await host.firstWindow();
    await expect.poll(() => host?.windows().length ?? 0).toBe(1);

    // The packaged Memoria UI must hydrate from ARK before capture works.
    await expect
      .poll(() => page.evaluate(() => Boolean(window.api)), { timeout: 30_000 })
      .toBe(true);

    // Всё (Everything) is the home grid; the add card starts a new note.
    const addCard = page.getByTestId("everything-add-card");
    await addCard.waitFor({ state: "visible", timeout: 30_000 });
    await addCard.click();

    const title = "KOS-93 smoke note";
    const body = "KOS-93 smoke body text";
    const titleInput = page.locator(".tiptap-title-input");
    await titleInput.waitFor({ state: "visible", timeout: 30_000 });
    await titleInput.fill(title);
    // Enter in the title moves focus to the start of the note body.
    await titleInput.press("Enter");
    const editor = page.locator(".tiptap-editor-content .ProseMirror");
    await editor.waitFor({ state: "visible" });
    await editor.click();
    await editor.pressSequentially(body);

    // The 300 ms autosave persists the draft through the Host-scoped ARK
    // bridge; `list_objects` is filtered by the app's typed grant on the
    // Engine side. Title and body can land in separate autosave passes.
    const listNotes = () =>
      page.evaluate(async () => JSON.stringify(await window.kosmosApp.ark.request("list_objects")));
    await expect.poll(listNotes, { timeout: 15_000 }).toContain(title);
    await expect.poll(listNotes, { timeout: 15_000 }).toContain(body);

    // Back in Всё (Everything) the new note is listed as a card. The section
    // nav is hidden while a note is open, so the titlebar history control
    // ("Назад") is the way back.
    await page.getByTestId("titlebar-history-back").click();
    const card = page.locator("[data-testid^='everything-card-']", { hasText: title });
    await card.waitFor({ state: "visible", timeout: 15_000 });
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
    expect(cleanupErrors, "Memoria smoke cleanup failed").toHaveLength(0);
  }
});
