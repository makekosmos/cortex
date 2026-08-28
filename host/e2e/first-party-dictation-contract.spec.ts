import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication } from "playwright";
import { test, expect } from "@playwright/test";
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
const forbiddenRendererKeys =
  /"(?:apiKey|credentialHandle|secret|token|password|localModelPath|localCommandPath|microphoneDeviceId|modelsDir|commandPath|path|url|filename|description|networkProfile|httpProxy|transcriptionPrompt|activeUuid|lastError)"\s*:/;

test("signed Dictation enforces its v2 contract in Host", async () => {
  test.setTimeout(180_000);
  if (!fs.existsSync(hostMain)) throw new Error(`build Host first: ${hostMain}`);
  const cleanupManifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!cleanupManifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-dictation-"));
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
    const apps = createSignedApps(
      root,
      workspaceRoot,
      false,
      false,
      false,
      false,
      false,
      false,
      true,
    );
    expect(apps.versions["com.kosmos.dictation"]).toBe("0.2.2");
    const archive = apps.archives["com.kosmos.dictation"];
    expect(archive).toBeTruthy();
    // SAFETY: createSignedApps serializes the catalog shape asserted below.
    const catalog = JSON.parse(apps.catalog) as {
      packages: Array<{
        manifest: { id: string; version: string; entrypoint: string; schema_version: number };
        sha256: string;
      }>;
    };
    const dictationPackage = catalog.packages.find(
      ({ manifest }) => manifest.id === "com.kosmos.dictation",
    );
    expect(dictationPackage).toMatchObject({
      manifest: {
        schema_version: 2,
        id: "com.kosmos.dictation",
        version: "0.2.2",
        entrypoint: "dist/index.html",
      },
      sha256: "8ed0d8b95c3c84f9a50b0efecb1005bdfb07d448dd23f9e83cdcc8625066aae5",
    });

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
          id: "com.kosmos.dictation",
          version: "0.2.2",
          archive_path: archive,
        })
      ).ok,
    ).toBe(true);
    expect(
      (
        await rpc(lock, "packages.set_enabled", {
          id: "com.kosmos.dictation",
          version: "0.2.2",
          enabled: true,
        })
      ).ok,
    ).toBe(true);

    host = await electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.dictation"],
      env: environment,
      timeout: 30_000,
    });
    pids.add(host.process().pid);
    const page = await host.firstWindow();
    await expect.poll(() => host?.windows().length ?? 0).toBe(1);
    expect(await page.evaluate(() => window.kosmosApp.identity)).toMatchObject({
      id: "com.kosmos.dictation",
      version: "0.2.2",
    });

    const responses = await page.evaluate(async () => {
      const request = window.kosmosApp.ark.request;
      return {
        state: await request("dictation.get_state", {}),
        config: await request("dictation.get_config", {}),
        models: await request("dictation.list_local_models", {}),
        updated: await request("dictation.update_config", { language: "en" }),
        denied: await request("dictation.submit_audio", { audioB64: "" }),
      };
    });
    expect(responses.state, JSON.stringify(responses.state)).toMatchObject({ ok: true });
    expect(responses.config, JSON.stringify(responses.config)).toMatchObject({ ok: true });
    expect(responses.models, JSON.stringify(responses.models)).toMatchObject({ ok: true });
    expect(responses.updated, JSON.stringify(responses.updated)).toMatchObject({ ok: true });
    expect(responses.denied, JSON.stringify(responses.denied)).toMatchObject({ ok: false });
    for (const response of [responses.state, responses.config, responses.models, responses.updated])
      expect(JSON.stringify(response)).not.toMatch(forbiddenRendererKeys);

    await expect(page.getByText("Настройки").first()).toBeVisible();
    const visibleRendererText = await page.locator("body").innerText();
    expect(visibleRendererText).not.toMatch(
      /api[_ -]?key|credential|secret|localModelPath|localCommandPath|microphoneDeviceId|hwnd/i,
    );
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
    expect(cleanupErrors, "Dictation E2E cleanup failed").toHaveLength(0);
  }
});
