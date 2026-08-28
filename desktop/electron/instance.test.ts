import { expect, mock, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

let appDataPath = "";
mock.module("electron", () => ({ app: { getPath: () => appDataPath } }));

const { migrateLegacyDevSettings, migrateLegacyProdSettings, productNameForSlot } =
  await import("./instance");

test("migrates legacy shell settings into the current data directory once", () => {
  const root = mkdtempSync(path.join(tmpdir(), "kosmos-instance-"));
  try {
    const targetDir = path.join(root, "Kosmos");
    const legacy = path.join(targetDir, "kepler-shell-settings.json");
    mkdirSync(path.dirname(legacy), { recursive: true });
    writeFileSync(legacy, '{"hotkey":"Control+Space"}', "utf8");

    migrateLegacyProdSettings(root, targetDir);
    const target = path.join(targetDir, "kosmos-settings.json");
    expect(existsSync(target)).toBe(true);
    expect(readFileSync(target, "utf8")).toBe('{"hotkey":"Control+Space"}');

    writeFileSync(target, '{"hotkey":"Alt+Space"}', "utf8");
    migrateLegacyProdSettings(root, targetDir);
    expect(readFileSync(target, "utf8")).toBe('{"hotkey":"Alt+Space"}');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("keeps the older Kepler settings fallback", () => {
  const root = mkdtempSync(path.join(tmpdir(), "kosmos-instance-kepler-"));
  try {
    const legacy = path.join(root, "Kepler", "kepler-shell-settings.json");
    const targetDir = path.join(root, "Kosmos");
    mkdirSync(path.dirname(legacy), { recursive: true });
    writeFileSync(legacy, '{"hotkey":"Control+Space"}', "utf8");

    migrateLegacyProdSettings(root, targetDir);
    expect(readFileSync(path.join(targetDir, "kosmos-settings.json"), "utf8")).toBe(
      '{"hotkey":"Control+Space"}',
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("replaces a malformed current file from the older Kepler fallback", () => {
  const root = mkdtempSync(path.join(tmpdir(), "kosmos-instance-corrupt-"));
  try {
    const targetDir = path.join(root, "Kosmos");
    const target = path.join(targetDir, "kosmos-settings.json");
    const legacy = path.join(root, "Kepler", "kepler-shell-settings.json");
    mkdirSync(targetDir, { recursive: true });
    mkdirSync(path.dirname(legacy), { recursive: true });
    writeFileSync(target, "{", "utf8");
    writeFileSync(legacy, '{"hotkey":"Control+Space"}', "utf8");

    migrateLegacyProdSettings(root, targetDir);
    expect(readFileSync(target, "utf8")).toBe('{"hotkey":"Control+Space"}');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("migrates dev settings into the data directory used by settings-store", () => {
  const root = mkdtempSync(path.join(tmpdir(), "kosmos-instance-dev-"));
  try {
    appDataPath = root;
    const legacy = path.join(root, "Kepler-dev", "kepler-shell-settings.json");
    const dataDir = path.join(root, "Kosmos-dev");
    mkdirSync(path.dirname(legacy), { recursive: true });
    writeFileSync(legacy, '{"hotkey":"Control+Space"}', "utf8");

    migrateLegacyDevSettings(dataDir);
    expect(readFileSync(path.join(dataDir, "kosmos-settings.json"), "utf8")).toBe(
      '{"hotkey":"Control+Space"}',
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("uses CosCast for user-facing instance names while preserving slot labels", () => {
  expect(productNameForSlot("prod", "prod")).toBe("CosCast");
  expect(productNameForSlot("dev", "dev")).toBe("CosCast [dev]");
  expect(productNameForSlot("test-example", "test")).toBe("CosCast [test]");
  expect(productNameForSlot("dev-review", "dev")).toBe("CosCast [dev-review]");
});
