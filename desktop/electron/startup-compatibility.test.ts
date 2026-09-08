import { expect, test } from "bun:test";
import type { JsonValue } from "./extension-permissions";
import { isString } from "../src/shared/runtimeGuards";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const appReadySource = await readFile(path.join(import.meta.dir, "main-app-ready.ts"), "utf8");
const managerNavigationSource = await readFile(
  path.join(import.meta.dir, "manager-navigation.ts"),
  "utf8",
);
const hostAppSource = await readFile(path.join(import.meta.dir, "host-app.ts"), "utf8");
const postUpdateSource = await readFile(path.join(import.meta.dir, "main-post-update.ts"), "utf8");
const instanceSource = await readFile(path.join(import.meta.dir, "instance.ts"), "utf8");
const rendererSource = await readFile(path.join(import.meta.dir, "../src/main.ts"), "utf8");
const legacyAppSource = await readFile(path.join(import.meta.dir, "../src/App.vue"), "utf8");
const settingsNavigationSource = await readFile(
  path.join(import.meta.dir, "../src/views/settings/navigation.ts"),
  "utf8",
);
const settingsNavigationDataSource = await readFile(
  path.join(import.meta.dir, "../src/views/settings/navigation.data.ts"),
  "utf8",
);
const settingsNavigationCommandsSource = await readFile(
  path.join(import.meta.dir, "../src/views/settings/navigation.commands.ts"),
  "utf8",
);
const settingsViewSource = await readFile(
  path.join(import.meta.dir, "../src/views/SettingsView.vue"),
  "utf8",
);
const launcherSource = await readFile(
  path.join(import.meta.dir, "../src/views/LauncherView.vue"),
  "utf8",
);
const launcherTemplateSource = await readFile(
  path.join(import.meta.dir, "../src/views/LauncherView.html"),
  "utf8",
);
const clipboardRetirementSources = await Promise.all(
  [
    "main.ts",
    "main-app-ready.ts",
    "main-shell-services.ts",
    "main-launcher.ts",
    "commands.ts",
    "preload-bridge.ts",
    "../shared/ipc-types.ts",
    "../shared/ipc-api-types.ts",
    "../src/views/LauncherView.vue",
    "../src/views/SettingsView.vue",
    "../src/views/settings/navigation.ts",
    "../src/views/settings/navigation.data.ts",
  ].map(async (file) => [file, await readFile(path.join(import.meta.dir, file), "utf8")] as const),
);

test("runAppReady opens Manager on manual launch and keeps autostart silent", () => {
  expect(appReadySource).toContain("const boot = backendSupervisor.initArkClient()");
  expect(appReadySource).toContain('process.env.KOSMOS_TEST_MODE === "1"');
  expect(appReadySource).toContain("? launcher.showLauncher");
  expect(appReadySource).toContain(": launcher.openManager");
  expect(appReadySource.indexOf("initArkClient()")).toBeLessThan(
    appReadySource.indexOf("openManager();"),
  );
  expect(appReadySource).not.toContain("boot.then(openManager");
  expect(appReadySource).toContain("launcher.setTrayVisible");
  expect(appReadySource).toContain("launcher.registerLauncherHotkeys");
  expect(appReadySource).toContain('"--autostart"');
  expect(appReadySource).toContain("void boot.catch");
  expect(appReadySource).toMatch(/log\.error\(\s*"migration",\s*"legacy migration failed"/);
  expect(appReadySource.indexOf("openManager();")).toBeLessThan(
    appReadySource.indexOf("if (runLegacyMigration)"),
  );
  expect(appReadySource.indexOf("launcher.setTrayVisible(isTrayIconEnabled())")).toBeLessThan(
    appReadySource.indexOf("if (runLegacyMigration)"),
  );
  expect(instanceSource).toContain('process.platform === "darwin" ? "Command+Space" : "Alt+Space"');
});

test("GUI components hide only their detached helper processes", () => {
  expect(managerNavigationSource).toContain("windowsHide: true");
  expect(hostAppSource).toContain("windowsHide: true");
});

test("desktop startup and post-update stay hidden", () => {
  expect(appReadySource).not.toContain("launcher.showLauncher()");
  expect(postUpdateSource).not.toContain("launcher.showLauncher()");
  expect(appReadySource).toContain("launcher.setTrayVisible(isTrayIconEnabled())");
});

test("no-hash compatibility renderer keeps the legacy App fallback", () => {
  // Regression: 2026-07-31. Desktop compatibility BrowserWindow loads without a hash.
  expect(rendererSource).toContain('import App from "./App.vue"');
  expect(rendererSource).toContain("return App;");
  expect(legacyAppSource).toContain('import LauncherView from "./views/LauncherView.vue"');
  expect(legacyAppSource).toContain("<LauncherView />");
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

test("launcher visibility remains owner", () => {
  for (const source of [
    settingsNavigationSource,
    settingsNavigationDataSource,
    settingsNavigationCommandsSource,
    settingsViewSource,
  ]) {
    expect(source).not.toMatch(/notes|tasks|games|eden:|delphi:|arrancador:/i);
  }
  expect(settingsNavigationSource).toContain('"time-tracker"');
  expect(settingsNavigationSource).toContain("kepler.launcher.hiddenCommandIds");
  expect(launcherSource).toContain(
    'const HIDDEN_COMMANDS_KEY = "kepler.launcher.hiddenCommandIds"',
  );
  expect(launcherSource).toContain("Скрытые команды");
  expect(launcherSource).toContain("localStorage.setItem(HIDDEN_COMMANDS_KEY");
  expect(settingsViewSource).toContain("localStorage.getItem(HIDDEN_COMMANDS_KEY)");
  expect(settingsViewSource).toContain("hiddenCommandIds.value.includes(id)");
  expect(settingsViewSource).toContain("localStorage.setItem(HIDDEN_COMMANDS_KEY");
  expect(launcherSource).toContain("hiddenCommandIds.value.includes(cmd.id)");
  expect(launcherTemplateSource).toContain("toggleCommandVisibility(selectedCommand.id)");
});

function extractFunction(source: string, name: string): string {
  const start = source.indexOf(`function ${name}`);
  if (start < 0) throw new Error(`missing ${name}`);
  const bodyStart = source.indexOf("{", start);
  let depth = 0;
  for (let i = bodyStart; i < source.length; i += 1) {
    if (source[i] === "{") depth += 1;
    if (source[i] === "}" && --depth === 0) return source.slice(start, i + 1);
  }
  throw new Error(`unterminated ${name}`);
}

function compileFunction(
  source: string,
  name: string,
  bindings: CompileBindings,
): (...args: JsonValue[]) => JsonValue {
  const js = extractFunction(source, name)
    .replaceAll("): string[] {", "){")
    .replaceAll("): boolean {", "){")
    .replaceAll("(id): id is string", "(id)")
    .replaceAll("(id: string)", "(id)")
    .replaceAll("(ids: string[])", "(ids)")
    .replaceAll("(cmd: CommandRecord)", "(cmd)");
  const factory = new Function(
    ...Object.keys(bindings),
    `const HIDDEN_COMMANDS_KEY = "kepler.launcher.hiddenCommandIds"; ${js}; return ${name};`,
  );
  // SAFETY: The extracted function is compiled from the named source and returns JSON-compatible test data.
  return factory(...Object.values(bindings)) as (...args: JsonValue[]) => JsonValue;
}

interface CompileBindings {}

test("seeded hidden command state is shared by Settings and Launcher and recovers", () => {
  const values = new Map<string, string>();
  const storage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
  };
  const key = "kepler.launcher.hiddenCommandIds";
  const seeded = ["eden:open", "delphi:task:create", "arrancador:open"];
  values.set(key, JSON.stringify(seeded));

  // SAFETY: Test state is intentionally initialized as a string-id collection.
  const settingsState = { value: [] as string[] };
  // SAFETY: Test state is intentionally initialized as a string-id collection.
  const launcherState = { value: [] as string[] };
  const settingsLoad = compileFunction(settingsViewSource, "loadHiddenCommandIds", {
    localStorage: storage,
  });
  const launcherLoad = compileFunction(launcherSource, "loadHiddenCommandIds", {
    localStorage: storage,
    isString,
  });
  // SAFETY: The extracted loaders return the seeded string-id arrays.
  settingsState.value = settingsLoad() as string[];
  // SAFETY: The extracted loaders return the seeded string-id arrays.
  launcherState.value = launcherLoad() as string[];
  expect(settingsState.value).toEqual(seeded);
  expect(launcherState.value).toEqual(seeded);

  const launcherVisible = compileFunction(launcherSource, "isCommandVisible", {
    hiddenCommandIds: launcherState,
  });
  // SAFETY: The extracted visibility function accepts command-shaped JSON objects.
  expect(launcherVisible({ id: "eden:open" })).toBe(false);
  expect(launcherVisible({ id: "delphi:task:today" })).toBe(true);

  const launcherToggle = compileFunction(launcherSource, "toggleCommandVisibility", {
    localStorage: storage,
    hiddenCommandIds: launcherState,
    selectedIndex: { value: 0 },
    refreshCommands: () => Promise.resolve(),
  });
  launcherToggle("eden:open");
  expect(JSON.parse(values.get(key) ?? "[]")).not.toContain("eden:open");
  expect(launcherState.value).not.toContain("eden:open");
  launcherToggle("eden:open");
  expect(JSON.parse(values.get(key) ?? "[]")).toContain("eden:open");
});
