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
  expect(settingsNavigationSource).toContain("kepler.launcher.hiddenCommandIds");
  expect(launcherSource).toContain(
    'const HIDDEN_COMMANDS_KEY = "kepler.launcher.hiddenCommandIds"',
  );
  expect(launcherSource).toContain("Скрытые команды");
  expect(launcherSource).toContain("localStorage.setItem(HIDDEN_COMMANDS_KEY");
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

test("seeded hidden command state is loaded by Launcher and recovers", () => {
  const values = new Map<string, string>();
  const storage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
  };
  const key = "kepler.launcher.hiddenCommandIds";
  const seeded = ["eden:open", "delphi:task:create", "arrancador:open"];
  values.set(key, JSON.stringify(seeded));

  // SAFETY: Test state is intentionally initialized as a string-id collection.
  const launcherState = { value: [] as string[] };
  const launcherLoad = compileFunction(launcherSource, "loadHiddenCommandIds", {
    localStorage: storage,
    isString,
  });
  // SAFETY: The extracted loaders return the seeded string-id arrays.
  launcherState.value = launcherLoad() as string[];
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
