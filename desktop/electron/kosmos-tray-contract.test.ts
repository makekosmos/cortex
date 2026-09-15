import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
const launcher = readFileSync(path.join(import.meta.dirname, "main-launcher.ts"), "utf8");
const tray = readFileSync(path.join(root, "runtime/src/backend_tray/windows_impl.rs"), "utf8");
const engine = readFileSync(path.join(root, "desktop/scripts/engine-distribution.mjs"), "utf8");

test("backend owns the single Windows tray contract", () => {
  expect(launcher).not.toContain("new Tray(");
  expect(tray).toContain('wide("Открыть")');
  expect(tray).toContain('wide("Выход")');
  expect(tray).toContain("resolve_cortex_executable().is_some()");
  expect(engine).toContain('"tray.ico"');
});
