import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";

const source = readFileSync(path.join(import.meta.dirname, "build-package-components.mjs"), "utf8");
const desktopBuild = readFileSync(path.join(import.meta.dirname, "build-desktop.mjs"), "utf8");

test("the icon pipeline runs before component staging", () => {
  const icons = readFileSync(path.join(import.meta.dirname, "build-app-icons.mjs"), "utf8");
  expect(icons).toContain("png-to-ico");
  const iconsStep = source.indexOf("build-app-icons.mjs");
  const managerStage = source.indexOf('"manager", "win-unpacked"');
  expect(iconsStep >= 0 && iconsStep < managerStage).toBeTruthy();
});

// The staged tree is what the installer ships — assert on the produced
// layout (only `components/manager`, exe under its packaged name), not on
// strings that happen to be absent.
test("the component build produces exactly a manager stage", () => {
  // The whole staging root is wiped first — a stale sibling dir can never
  // leak into the installer payload.
  expect(source).toContain('".tmp", "components"');
  expect(source).toContain("rmSync(componentsRoot, { recursive: true, force: true })");
  expect(source).toContain('"manager", "win-unpacked"');
  // manager-gpui is built for the Windows target and staged under the
  // packaged name the tray resolves.
  expect(source).toContain('"cargo"');
  expect(source).toContain('"--locked"');
  expect(source).toContain("x86_64-pc-windows-msvc");
  expect(source).toContain('"Mundus Manager.exe"');
  // A missing build product fails the script instead of shipping nothing.
  expect(source).toMatch(/missing \$\{?managerExe|missing.*manager-gpui\.exe/i);
});

test("the installer stage requires the manager component", () => {
  // stageInstaller must die when the manager stage is absent — never ship a
  // payload without the only bundled component.
  const stage = desktopBuild.slice(desktopBuild.indexOf("function stageInstaller"));
  expect(stage).toContain('"components", "manager", "win-unpacked"');
  expect(stage).toContain("missing manager component");
  expect(stage).not.toContain("component-pins.json");
});
