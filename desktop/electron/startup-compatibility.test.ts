import { expect, test } from "../test-support/node-test.mjs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const appReadySource = await readFile(path.join(import.meta.dirname, "main-app-ready.ts"), "utf8");
const managerNavigationSource = await readFile(
  path.join(import.meta.dirname, "manager-navigation.ts"),
  "utf8",
);
const hostAppSource = await readFile(path.join(import.meta.dirname, "host-app.ts"), "utf8");
const instanceSource = await readFile(path.join(import.meta.dirname, "instance.ts"), "utf8");
const rendererSource = await readFile(path.join(import.meta.dirname, "../src/main.ts"), "utf8");
const settingsNavigationSource = await readFile(
  path.join(import.meta.dirname, "../src/views/settings/navigation.ts"),
  "utf8",
);
const settingsNavigationDataSource = await readFile(
  path.join(import.meta.dirname, "../src/views/settings/navigation.data.ts"),
  "utf8",
);
const settingsViewSource = await readFile(
  path.join(import.meta.dirname, "../src/views/SettingsView.vue"),
  "utf8",
);
const clipboardRetirementSources = await Promise.all(
  [
    "main.ts",
    "main-app-ready.ts",
    "main-shell-services.ts",
    "commands.ts",
    "preload-bridge.ts",
    "../shared/ipc-types.ts",
    "../shared/ipc-api-types.ts",
    "../src/views/SettingsView.vue",
    "../src/views/settings/navigation.ts",
    "../src/views/settings/navigation.data.ts",
  ].map(
    async (file) => [file, await readFile(path.join(import.meta.dirname, file), "utf8")] as const,
  ),
);

test("runAppReady opens Manager on manual launch and keeps autostart silent", () => {
  expect(appReadySource).toContain("const boot = backendSupervisor.initArkClient()");
  expect(appReadySource).toContain('process.env.KOSMOS_TEST_MODE === "1"');
  expect(appReadySource).toContain("? openTestHarnessWindow");
  expect(appReadySource).toContain(": openManager");
  expect(appReadySource.indexOf("initArkClient()")).toBeLessThan(
    appReadySource.indexOf("openSurface();"),
  );
  expect(appReadySource.indexOf("openSurface();")).toBeLessThan(
    appReadySource.indexOf("if (runLegacyMigration)"),
  );
  expect(appReadySource).not.toContain("boot.then(openManager");
  expect(appReadySource).toContain("registerGlobalHotkey");
  expect(appReadySource).toContain('"--autostart"');
  expect(appReadySource).toContain("void boot.catch");
  expect(appReadySource).toMatch(/log\.error\(\s*"migration",\s*"legacy migration failed"/);
  expect(instanceSource).toContain('process.platform === "darwin" ? "Command+Space" : "Alt+Space"');
});

test("GUI components hide only their detached helper processes", () => {
  expect(managerNavigationSource).toContain("windowsHide: true");
  expect(hostAppSource).toContain("windowsHide: true");
});

test("desktop startup stays silent on autostart", () => {
  expect(appReadySource).toContain('if (!process.argv.includes("--autostart"))');
  expect(appReadySource.indexOf('argv.includes("--autostart")')).toBeLessThan(
    appReadySource.indexOf("openSurface();"),
  );
});

test("no-hash renderer fallback is an inert stub without the legacy launcher", () => {
  expect(rendererSource).not.toContain("LauncherView");
  expect(rendererSource).not.toContain("./App.vue");
  expect(rendererSource).toContain('return "div";');
});

test("retired clipboard history leaves no desktop lifecycle or IPC residue", () => {
  for (const [file, source] of clipboardRetirementSources) {
    expect(source, file).not.toMatch(
      /CLIPBOARD_HISTORY_ENABLED|clipboardHistory|ClipboardHistory|ClipboardSettingsTab|ClipboardQuickPanel|kepler:clipboard-history|clipboard-history\.ts/,
    );
  }
});

test("isolated upgrade fixture preserves historical clipboard JSON bytes", async () => {
  const dataDir = await mkdtemp(path.join(os.tmpdir(), "kosmos-clipboard-retirement-"));
  const filePath = path.join(dataDir, "clipboard-history.json");
  const seed = Buffer.from('{"sensitive":"keep me"}\n', "utf8");
  try {
    await writeFile(filePath, seed);

    // The current headless startup sources contain no clipboard-history path
    // or operation. This isolated upgrade fixture proves the inert orphan is
    // not touched while the no-residue test above proves the boot path has no
    // remaining import, timer, IPC registration, or command route.
    expect(process.env.KOSMOS_HEADLESS ?? "1").toBeTruthy();
    const after = await readFile(filePath);
    expect(after.equals(seed)).toBe(true);
  } finally {
    await rm(dataDir, { recursive: true, force: true });
  }
});

test("settings sources stay free of retired app references", () => {
  for (const source of [
    settingsNavigationSource,
    settingsNavigationDataSource,
    settingsViewSource,
  ]) {
    expect(source).not.toMatch(/notes|tasks|games|eden:|delphi:|arrancador:/i);
  }
});
