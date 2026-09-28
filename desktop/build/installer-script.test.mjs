import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const buildDir = path.dirname(fileURLToPath(import.meta.url));
const installer = readFileSync(path.join(buildDir, "installer.nsi"), "utf8");

const installSection =
  installer.split('Section "Install"')[1]?.split('Section "Uninstall"')[0] ?? "";
const uninstallSection = installer.split('Section "Uninstall"')[1] ?? "";
const macroSection = installer.split("!macro KillKosmosProcesses")[1] ?? "";

const runKey = "${RUN_KEY}";

test("uses a fixed install directory and never lets the user choose", () => {
  expect(installer).not.toContain("Page directory");
});

test("only removes the shipped payload, not the whole $INSTDIR recursively", () => {
  const recursiveInstDir = installer.match(/^RMDir\s+\/r\s+"\$INSTDIR"\s*$/m);
  expect(recursiveInstDir).toBeNull();
  expect(installer).toContain('RMDir /r "$INSTDIR\\resources"');
  expect(installer).toContain('RMDir "$INSTDIR"');
});

test("cleans up the old Electron install under kepler-shell and its GUID keys", () => {
  expect(installSection).toContain('RMDir /r "$LOCALAPPDATA\\Programs\\kepler-shell"');
  expect(installSection).toContain("{4fe2b964-4d0e-5a72-8728-cca14468c9f0}");
  expect(installSection).toContain("{af85bd72-f4c8-5af3-a0fe-9aa1f0fa5a8d}");
});

test("removes old Electron autostart Run values", () => {
  const names = [
    "com.kazui.kosmos",
    "electron.app.Kosmos",
    "Kosmos",
    "com.kazui.kepler",
    "Kepler",
    "KeplerKosmos",
    "KosmosKepler",
  ];
  for (const name of names) {
    expect(installer).toContain(`DeleteRegValue HKCU "${runKey}" "${name}"`);
  }
});

test("stops processes before uninstalling", () => {
  expect(uninstallSection).toContain("!insertmacro KillKosmosProcesses");
  expect(macroSection).toContain("taskkill /F /IM kepler-backend.exe");
  expect(macroSection).toContain('taskkill /F /IM "Kosmos Manager.exe"');
  expect(macroSection).toContain("Sleep 500");
});

test("always starts the Engine at the end of install", () => {
  expect(installer).toContain("kepler-backend.exe");
  expect(installer).toContain("--start");
  expect(installer).toContain("Start-Process");
});

test("only opens the Manager on interactive installs", () => {
  expect(installer).toContain(
    "Exec '\"$INSTDIR\\resources\\components\\manager\\Kosmos Manager.exe\"'",
  );
  expect(installer).toContain("IfSilent");
});

test("seeds autostart only conditionally and never using the Desktop VERSION", () => {
  expect(installer).not.toContain("versions\\${VERSION}\\kepler-backend.exe");
  expect(installSection).not.toContain('WriteRegStr HKCU "${RUN_KEY}" "Kosmos Engine"');
});

test("detects previous installs and migrates autostart via dedicated functions", () => {
  expect(installer).toContain("Function DetectPreviousKosmos");
  expect(installer).toContain("Function SeedOrMigrateAutostart");
  expect(installer).toContain("Function StartEngineAndManager");
});

test("ignores legacy electron-updater arguments", () => {
  expect(installer).toContain("Function .onInit");
  expect(installer).toMatch(/--updated|--force-run/);
});
