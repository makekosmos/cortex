// Small smoke: one Electron per run, managed Vite in dev and the packaged app separately.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication, type Page } from "playwright";
import { freshDataDir, launchKeplerWithDataDir, shutdownKeplerEngine } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";
import { readRunManifest } from "../scripts/dev-run-manifest.mjs";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const managedManifestPath = process.env.KOSMOS_DEV_RUN_MANIFEST;
const testRoot = path.join(appRoot, ".e2e", "runs");
const managedManifest = managedManifestPath
  ? readRunManifest(managedManifestPath, testRoot)
  : undefined;
const dataDirs = new Set<string>();

async function launchKepler(): Promise<{
  app: ElectronApplication;
  launcher: Page;
  dataDir: string;
}> {
  const dataDir = managedManifest?.dataDir ?? freshDataDir("smoke");
  const userDataDir = managedManifest?.userDataDir;
  dataDirs.add(dataDir);
  const filePath = path.join(dataDir, "clipboard-history.json");
  fs.mkdirSync(dataDir, { recursive: true });
  fs.writeFileSync(filePath, '{"sensitive":"keep me"}\n', "utf8");
  const packagedRoot = process.env.KOSMOS_PACKAGED_ROOT;
  let app: ElectronApplication;
  if (!packagedRoot) {
    app = await launchKeplerWithDataDir(
      dataDir,
      { KEPLER_DEV: "1", VITE_DEV_SERVER_URL: process.env.VITE_DEV_SERVER_URL },
      userDataDir,
      managedManifest?.runId,
    );
  } else {
    const env = {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      KEPLER_INSTANCE: `test-${path.basename(dataDir).toLowerCase()}`,
      KOSMOS_TEST_MODE: "1",
      KOSMOS_HEADLESS: "1",
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KEPLER_SKIP_SYNC: "1",
    };
    delete env.KEPLER_BACKEND_EXE;
    delete env.KOSMOS_ENGINE_ROOT;
    delete env.VITE_DEV_SERVER_URL;
    app = await electron.launch({
      executablePath: path.join(packagedRoot, "Kosmos.exe"),
      cwd: packagedRoot,
      args: [`--user-data-dir=${userDataDir ?? path.join(dataDir, "userdata")}`],
      env,
      timeout: 20_000,
    });
  }
  const launcher = app.windows()[0] ?? (await app.waitForEvent("window", { timeout: 20_000 }));
  await launcher.waitForLoadState("domcontentloaded");
  if (!packagedRoot) {
    const devUrl = process.env.VITE_DEV_SERVER_URL;
    expect(devUrl).toBeTruthy();
    expect(new URL(launcher.url()).origin).toBe(new URL(devUrl!).origin);
  } else expect(launcher.url()).not.toContain("dist/index.html");
  await waitForBackendReady(launcher);
  return { app, launcher, dataDir };
}

test.afterEach(() => {
  for (const dataDir of dataDirs) {
    shutdownKeplerEngine(dataDir);
    if (!managedManifest) fs.rmSync(dataDir, { recursive: true, force: true });
  }
  dataDirs.clear();
});

test("shell, IPC, and clipboard smoke use one Electron", async () => {
  const { app, launcher, dataDir } = await launchKepler();
  const filePath = path.join(dataDir, "clipboard-history.json");
  const seed = Buffer.from('{"sensitive":"keep me"}\n', "utf8");
  try {
    const appPath = await app.evaluate(({ app: electronApp }) => electronApp.getAppPath());
    if (!process.env.KOSMOS_PACKAGED_ROOT) {
      expect(path.basename(appPath)).toBe("dist-electron");
      expect(path.normalize(path.dirname(appPath))).toBe(path.normalize(appRoot));
    }
    expect(await app.evaluate(({ app: electronApp }) => electronApp.getName())).toBe(
      "Kosmos [test]",
    );
    const windows = await app.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows().map((window) => ({
        isVisible: window.isVisible(),
        isDestroyed: window.isDestroyed(),
      })),
    );
    expect(windows.length).toBeGreaterThanOrEqual(1);
    expect(windows[0].isDestroyed).toBe(false);
    const version = await launcher.evaluate(() => window.kepler.settings.version());
    expect(Object.prototype.toString.call(version)).toBe("[object String]");
    expect(String(version).length).toBeGreaterThan(0);
  } finally {
    await app.close();
    expect(fs.readFileSync(filePath).equals(seed)).toBe(true);
  }
});
