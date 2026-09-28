import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const buildDir = path.dirname(fileURLToPath(import.meta.url));
const installer = readFileSync(path.join(buildDir, "installer.nsi"), "utf8");

const installSection =
  installer.split('Section "Install"')[1]?.split('Section "Uninstall"')[0] ?? "";
const uninstallSection = installer.split('Section "Uninstall"')[1] ?? "";
const macroSection = installer.split("!macro KillProductProcesses")[1] ?? "";

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
  // The real 0.9.x uninstall keys are bare GUIDs — no braces.
  expect(installer).toContain("Uninstall\\4fe2b964-4d0e-5a72-8728-cca14468c9f0");
  expect(installer).toContain("Uninstall\\af85bd72-f4c8-5af3-a0fe-9aa1f0fa5a8d");
  expect(installer).not.toContain("{4fe2b964");
  expect(installer).not.toContain("{af85bd72");
});

test("removes old autostart Run values unconditionally on install", () => {
  // MIGRATION(KOS-267): the legacy names must keep being deleted.
  const names = [
    "com.kazui.kosmos",
    "electron.app.Kosmos",
    "Kosmos",
    "Kosmos Engine",
    "com.kazui.kepler",
    "Kepler",
    "KeplerKosmos",
    "KosmosKepler",
  ];
  for (const name of names) {
    expect(installer).toContain(`DeleteRegValue HKCU "${runKey}" "${name}"`);
  }
  // Deletion happens before the legacy program-dir cleanup, not only inside it.
  const insertAt = installSection.indexOf("!insertmacro DeleteOldRunValues");
  const legacyProgramDirAt = installSection.indexOf(
    'RMDir /r "$LOCALAPPDATA\\Programs\\kepler-shell"',
  );
  expect(insertAt >= 0).toBeTruthy();
  expect(legacyProgramDirAt >= 0).toBeTruthy();
  expect(insertAt).toBeLessThan(legacyProgramDirAt);
});

test("stops processes before uninstalling", () => {
  expect(uninstallSection).toContain("!insertmacro KillProductProcesses");
  expect(macroSection).toContain("taskkill /F /IM mundus-engine.exe");
  expect(macroSection).toContain('taskkill /F /IM "Mundus Manager.exe"');
  // MIGRATION(KOS-267): the running pre-upgrade processes carry old names.
  expect(macroSection).toContain("taskkill /F /IM kepler-backend.exe");
  // 0.9.x installs leave an orphaned ark-core-rpc.exe holding the DB.
  expect(macroSection).toContain("taskkill /F /IM ark-core-rpc.exe");
  expect(macroSection).toContain('taskkill /F /IM "Kosmos Manager.exe"');
  expect(macroSection).toContain("taskkill /F /IM Kosmos.exe");
  expect(macroSection).toContain("Sleep 500");
});

test("always starts the Engine at the end of install", () => {
  const postInstall = readFileSync(path.join(buildDir, "engine-post-install.ps1"), "utf8");
  expect(postInstall).toContain("mundus-engine.exe");
  expect(postInstall).toContain("'Mundus Engine'");
  expect(postInstall).toContain("--start");
  expect(postInstall).toContain("Start-Process");
  expect(installer).toContain("engine-post-install.ps1");
});

test("only opens the Manager on interactive installs", () => {
  expect(installer).toContain(
    "Exec '\"$INSTDIR\\resources\\components\\manager\\${MANAGER_EXE}\"'",
  );
  expect(installer).toContain("IfSilent");
});

test("seeds autostart only conditionally and never using the Desktop VERSION", () => {
  expect(installer).not.toContain("versions\\${VERSION}\\mundus-engine.exe");
  expect(installSection).not.toContain('WriteRegStr HKCU "${RUN_KEY}" "Mundus Engine"');
});

test("post-install logic ships as a script file, never inline -Command", () => {
  expect(installer).not.toContain("-Command");
  expect(installer).toContain(
    '-File "$INSTDIR\\resources\\engine-post-install.ps1" -SeedAutostart',
  );
  expect(installer).toContain('-File "$INSTDIR\\resources\\engine-post-install.ps1" -StartEngine');
});

test("no NSIS single-quoted string contains '' (NSIS has no doubled-quote escape)", () => {
  expect(installer).not.toContain("''");
});

test("detects previous installs and migrates autostart via dedicated functions", () => {
  expect(installer).toContain("Function DetectPreviousInstall");
  expect(installer).toContain("Function SeedOrMigrateAutostart");
  expect(installer).toContain("Function StartEngineAndManager");
});

test("ignores legacy electron-updater arguments", () => {
  expect(installer).toContain("Function .onInit");
  expect(installer).toMatch(/--updated|--force-run/);
});
