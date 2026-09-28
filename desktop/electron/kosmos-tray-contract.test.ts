import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
const shell = readFileSync(path.join(import.meta.dirname, "main.ts"), "utf8");
const spawn = readFileSync(path.join(import.meta.dirname, "main-backend-process.ts"), "utf8");
const tray = readFileSync(path.join(root, "runtime/src/backend_tray/windows_impl.rs"), "utf8");
const engine = readFileSync(path.join(root, "desktop/scripts/engine-distribution.mjs"), "utf8");
const install = readFileSync(path.join(root, "desktop/build/install-engine.ps1"), "utf8");

test("backend owns the single Windows tray contract", () => {
  expect(shell).not.toContain("new Tray(");
  expect(tray).toContain('wide("Открыть")');
  expect(tray).toContain('wide("Выход")');
  expect(tray).toContain("resolve_cortex_executable().is_some()");
  expect(engine).toContain('"tray.ico"');
});

test("the tray icon asset reaches the backend regardless of Engine payload age", () => {
  // GUI hands the icon to the runtime: the backend also runs under installed
  // Engine builds whose versions/<v>/ directory predates tray.ico.
  expect(spawn).toContain("KOSMOS_TRAY_ICON");
  expect(tray).toContain('env::var_os("KOSMOS_TRAY_ICON")');
  // Install-time file verification is generic over whatever the manifest
  // lists (no hardcoded engine file allowlist), so a tray-capable Engine
  // payload is accepted the same way as any other.
  expect(install).toContain("$expected.files");
});
