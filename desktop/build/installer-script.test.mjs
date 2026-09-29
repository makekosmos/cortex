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

test("cleans up the old Electron-era payloads and their GUID keys", () => {
  expect(installSection).toContain('RMDir /r "$LOCALAPPDATA\\Programs\\kepler-shell"');
  // The Kosmos-era Programs payload goes too.
  expect(installSection).toContain('RMDir /r "$LOCALAPPDATA\\Programs\\Kosmos"');
  // The real 0.9.x uninstall keys are bare GUIDs — no braces.
  expect(installer).toContain("Uninstall\\4fe2b964-4d0e-5a72-8728-cca14468c9f0");
  expect(installer).toContain("Uninstall\\af85bd72-f4c8-5af3-a0fe-9aa1f0fa5a8d");
  expect(installer).not.toContain("{4fe2b964");
  expect(installer).not.toContain("{af85bd72");
});

test("deletes the brace-less Electron uninstall keys unconditionally", () => {
  // MIGRATION(KOS-267): the legacy keys must keep being deleted whether or
  // not the old payload dir survived.
  for (const guid of [
    "4fe2b964-4d0e-5a72-8728-cca14468c9f0",
    "af85bd72-f4c8-5af3-a0fe-9aa1f0fa5a8d",
  ]) {
    expect(installSection).toContain(
      `DeleteRegKey HKCU "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${guid}"`,
    );
  }
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

test("stops product processes — bundled, store apps and legacy names", () => {
  expect(uninstallSection).toContain("!insertmacro KillProductProcesses");
  expect(installSection).toContain("!insertmacro KillProductProcesses");
  expect(macroSection).toContain("taskkill /F /IM mundus-engine.exe");
  expect(macroSection).toContain('taskkill /F /IM "Mundus Manager.exe"');
  // Store-installed apps must be stopped before their dir is removed or
  // replaced.
  for (const exe of ["agenda-gpui.exe", "memoria-gpui.exe", "dictation-gpui.exe"]) {
    expect(macroSection).toContain(`taskkill /F /IM ${exe}`);
  }
  // MIGRATION(KOS-267): the running pre-upgrade processes carry old names.
  expect(macroSection).toContain("taskkill /F /IM kepler-backend.exe");
  expect(macroSection).toContain("taskkill /F /IM ark-core-rpc.exe");
  expect(macroSection).toContain('taskkill /F /IM "Kosmos Manager.exe"');
  expect(macroSection).toContain("taskkill /F /IM Kosmos.exe");
  expect(macroSection).toContain("Sleep 500");
});

test("uninstall removes the privileged service via one elevated runas call", () => {
  // Detection is unelevated (sc query), the actual teardown is delegated to
  // the stable service copy's `privileged uninstall` — NSIS stays thin.
  expect(installer).toContain("Function un.RemovePrivilegedService");
  expect(installer).toContain('sc.exe query "${PRIVILEGED_SVC_NAME}"');
  expect(installer).toContain('ExecShell "runas" "${PRIVILEGED_SVC_EXE}" "privileged uninstall"');
  // One prompt, only when the service is registered, and a user-visible note
  // when elevation is declined.
  expect(installer).toContain("elevation declined");
  expect(uninstallSection).toContain("Call un.RemovePrivilegedService");
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

// KOS-265: the install payload ships Manager only — `File` ships whatever
// build-desktop.mjs staged under resources\components (see the e2e test),
// and the installer itself must never write or shortcut an app component.
test("the Install section never writes or shortcuts a bundled app component", () => {
  for (const component of ["agenda", "memoria", "dictation"]) {
    // Component dirs may legitimately appear in RecordLegacyComponents
    // probes (read-only IfFileExists) — but never in a payload or shortcut
    // statement.
    expect(installSection).not.toContain(`components\\${component}\\`);
    expect(installSection).not.toContain(`CreateShortCut "$SMPROGRAMS\\${component}`);
  }
  expect(installSection).toContain('RMDir /r "$INSTDIR\\resources"');
});

// The migration marker must be recorded before the payloads it describes
// are deleted — a marker written after the wipe would always read "absent".
test("records bundled components before wiping the old payloads", () => {
  const marker = installer.split("Function RecordLegacyComponents")[1] ?? "";
  expect(marker).toContain("legacy-components.json");
  for (const component of ["agenda", "memoria", "dictation"]) {
    expect(marker).toContain(`components\\${component}`);
    expect(marker).toContain(`"${component}":$`);
  }
  const recordAt = installSection.indexOf("Call RecordLegacyComponents");
  const kosmosWipeAt = installSection.indexOf('RMDir /r "$LOCALAPPDATA\\Programs\\Kosmos"');
  const resourcesWipeAt = installSection.indexOf('RMDir /r "$INSTDIR\\resources"');
  expect(recordAt >= 0).toBeTruthy();
  expect(kosmosWipeAt >= 0).toBeTruthy();
  expect(resourcesWipeAt >= 0).toBeTruthy();
  expect(recordAt).toBeLessThan(kosmosWipeAt);
  expect(recordAt).toBeLessThan(resourcesWipeAt);
});

test("uninstall removes the store payload dir but never user data", () => {
  expect(uninstallSection).toContain('RMDir /r "${APPS_ROOT}"');
  // User data under the Mundus roots is deliberately kept.
  expect(uninstallSection).not.toContain('RMDir /r "$APPDATA\\Mundus"');
  expect(uninstallSection).not.toContain('RMDir /r "$LOCALAPPDATA\\Mundus"');
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
