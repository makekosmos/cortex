import { expect, mock, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

mock.module("electron", () => ({ app: {} }));

const { migrateLegacyProdSettings } = await import("./instance");

test("migrates legacy shell settings into the current data directory once", () => {
  const root = mkdtempSync(path.join(tmpdir(), "kosmos-instance-"));
  try {
    const legacy = path.join(root, "Kepler", "kepler-shell-settings.json");
    const targetDir = path.join(root, "Kosmos");
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
