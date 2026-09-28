import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
const shell = readFileSync(path.join(import.meta.dirname, "main.ts"), "utf8");
const spawn = readFileSync(path.join(import.meta.dirname, "main-backend-process.ts"), "utf8");
const trayRoot = path.join(root, "runtime/src/backend_tray/windows_impl");
const tray = readFileSync(path.join(trayRoot, "mod.rs"), "utf8");
const trayMenu = readFileSync(path.join(trayRoot, "menu.rs"), "utf8");
const trayComponents = readFileSync(path.join(trayRoot, "components.rs"), "utf8");
const trayPaths = readFileSync(path.join(trayRoot, "paths.rs"), "utf8");
const engine = readFileSync(path.join(root, "desktop/scripts/engine-distribution.mjs"), "utf8");
const install = readFileSync(path.join(root, "desktop/build/install-engine.ps1"), "utf8");
const managerNav = readFileSync(path.join(import.meta.dirname, "manager-navigation.ts"), "utf8");
const agendaNav = readFileSync(path.join(import.meta.dirname, "agenda-navigation.ts"), "utf8");
const memoriaNav = readFileSync(path.join(import.meta.dirname, "memoria-navigation.ts"), "utf8");

test("backend owns the single Windows tray contract", () => {
  expect(shell).not.toContain("new Tray(");
  expect(trayMenu).toContain('"Открыть"');
  expect(trayMenu).toContain('"Выход"');
  expect(tray).toContain("resolve_cortex_executable().is_some()");
  expect(engine).toContain('"tray.ico"');
});

test("the Engine tray reaches parity with the packaged GPUI components", () => {
  // KOS-236: the Engine tray launches the same Manager/Agenda/Memoria
  // executables the Electron shell resolved, under
  // resources/components/<name>/Kosmos <Name>.exe next to Cortex.
  for (const name of ["Manager", "Agenda", "Memoria"]) {
    expect(trayComponents).toContain(`"Kosmos ${name}.exe"`);
  }
  expect(trayComponents).toContain("KOSMOS_MANAGER_EXECUTABLE");
  expect(trayComponents).toContain("KOSMOS_AGENDA_EXECUTABLE");
  expect(trayComponents).toContain("KOSMOS_MEMORIA_EXECUTABLE");
  expect(trayPaths).toContain("resources");
  expect(trayPaths).toContain("components");
  // Same packaged-exe naming convention as the Electron navigation modules.
  expect(managerNav).toContain('"Kosmos Manager.exe"');
  expect(agendaNav).toContain('"Kosmos Agenda.exe"');
  expect(memoriaNav).toContain('"Kosmos Memoria.exe"');
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
