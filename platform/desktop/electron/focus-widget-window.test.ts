import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import path from "node:path";

// Regression: 2026-07-10. DWM background materials paint an opaque backing
// surface behind transparent pixels around the rounded Focus widget on Win32.
test("focus widget keeps its native backing surface transparent", async () => {
  const source = await readFile(path.join(import.meta.dir, "focus-widget-window.ts"), "utf8");
  const createWindow = source.match(
    /export function createFocusWidgetWindow[\s\S]*?(?=\n}\n?$)/,
  )?.[0];

  expect(createWindow).toBeDefined();
  expect(createWindow).toContain("transparent: true");
  expect(createWindow).toContain('backgroundColor: "#00000000"');
  expect(source).not.toContain("backgroundMaterialOption(");
  expect(source).not.toContain("applyWindowMaterial(");
  expect(createWindow).not.toMatch(/\bbackgroundMaterial\s*:/);
  expect(createWindow).not.toContain("setBackgroundMaterial(");
});
