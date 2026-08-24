import { expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { resolveExtensionWindowIcon } from "./extension-window-icon";

test("uses only an existing icon inside the extension directory", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "kosmos-extension-icon-"));
  try {
    const icon = path.join(dir, "icon.png");
    const kosmosIcon = path.join(dir, "kosmos.png");
    writeFileSync(icon, "fixture");
    writeFileSync(kosmosIcon, "fixture");
    expect(resolveExtensionWindowIcon(dir, "icon.png")).toBe(icon);
    expect(resolveExtensionWindowIcon(dir, "icon.png", kosmosIcon)).toBe(kosmosIcon);
    expect(resolveExtensionWindowIcon(dir, "../icon.png")).toBeUndefined();
    expect(resolveExtensionWindowIcon(dir, "missing.png")).toBeUndefined();
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
