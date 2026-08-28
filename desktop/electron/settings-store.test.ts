import { beforeEach, afterAll, expect, mock, test } from "bun:test";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const root = mkdtempSync(path.join(tmpdir(), "kosmos-settings-"));
const dataDir = path.resolve(root, "Kosmos");
mkdirSync(dataDir, { recursive: true });

mock.module("./data-dir", () => ({ keplerDataDir: () => dataDir }));
mock.module("./instance", () => ({
  resolveInstance: () => ({ hotkey: "Alt+Space" }),
}));

const { LEGACY_SETTINGS_FILE_NAME, SETTINGS_FILE_NAME, readSettings, writeSettings } =
  await import("./settings-store");

const currentPath = path.join(dataDir, SETTINGS_FILE_NAME);
const legacyPath = path.join(dataDir, LEGACY_SETTINGS_FILE_NAME);

beforeEach(() => {
  rmSync(currentPath, { force: true });
  rmSync(legacyPath, { force: true });
});

afterAll(() => {
  rmSync(root, { recursive: true, force: true });
});

test("fresh installs write only the Kosmos-named settings file", () => {
  expect(readSettings()).toEqual({});

  writeSettings({ hotkey: "Control+Space", showTrayIcon: true });

  expect(existsSync(currentPath)).toBe(true);
  expect(existsSync(legacyPath)).toBe(false);
  expect(JSON.parse(readFileSync(currentPath, "utf8"))).toEqual({
    hotkey: "Control+Space",
    showTrayIcon: true,
  });
});

test("legacy settings are read and promoted on the next write", () => {
  const legacy = { hotkey: "Alt+Space", showTrayIcon: false };
  writeFileSync(legacyPath, JSON.stringify(legacy), "utf8");

  expect(readSettings()).toEqual(legacy);

  writeSettings({ launcherStateTtlMinutes: 15 });

  expect(JSON.parse(readFileSync(currentPath, "utf8"))).toEqual({
    ...legacy,
    launcherStateTtlMinutes: 15,
  });
  expect(JSON.parse(readFileSync(legacyPath, "utf8"))).toEqual(legacy);
});

test("the Kosmos settings file takes precedence over the legacy file", () => {
  writeFileSync(legacyPath, JSON.stringify({ hotkey: "Alt+Space" }), "utf8");
  writeFileSync(currentPath, JSON.stringify({ hotkey: "Control+Space" }), "utf8");

  expect(readSettings()).toEqual({ hotkey: "Control+Space" });
});

test("structurally invalid current settings fall back without losing legacy values", () => {
  writeFileSync(legacyPath, JSON.stringify({ hotkey: "Alt+Space" }), "utf8");

  for (const invalid of ["null", "[]"]) {
    writeFileSync(currentPath, invalid, "utf8");
    expect(readSettings()).toEqual({ hotkey: "Alt+Space" });
  }

  writeSettings({ showTrayIcon: false });
  expect(JSON.parse(readFileSync(currentPath, "utf8"))).toEqual({
    hotkey: "Alt+Space",
    showTrayIcon: false,
  });
});
