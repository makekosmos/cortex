import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const source = readFileSync("platform/desktop/electron/main.ts", "utf8");

describe("launcher macOS window contract", () => {
  test("does not use Windows always-on-top show path on macOS", () => {
    // Regression: 2026-06-07. macOS packaged launcher became active without
    // a visible/reactive window after the Windows-oriented always-on-top path.
    expect(source).toContain('alwaysOnTop: process.platform === "win32"');
    expect(source).toContain('if (process.platform === "darwin")');
    expect(source).toContain("mainWindow.show();");
    expect(source).toContain("app.focus({ steal: true });");
    expect(source).toContain("mainWindow.moveTop();");
    expect(source).toContain("} else if (!mainWindow.isVisible())");
    expect(source).toContain("mainWindow.showInactive();");
  });
});
