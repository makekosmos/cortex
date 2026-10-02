import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const buildDir = path.dirname(fileURLToPath(import.meta.url));
const installer = readFileSync(path.join(buildDir, "installer.nsi"), "utf8");

const installSection =
  installer.split('Section "Install"')[1]?.split('Section "Uninstall"')[0] ?? "";
const uninstallSection = installer.split('Section "Uninstall"')[1] ?? "";
const macroSection = installer.split("!macro StopProductProcesses")[1] ?? "";

const runKey = "${RUN_KEY}";

test("uses MUI2 pages: welcome, progress, finish; no directory or license page", () => {
  expect(installer).toContain("!include MUI2.nsh");
  expect(installer).toContain("!insertmacro MUI_PAGE_WELCOME");
  expect(installer).toContain("!insertmacro MUI_PAGE_INSTFILES");
  expect(installer).toContain("!insertmacro MUI_PAGE_FINISH");
  expect(installer).toContain("!insertmacro MUI_UNPAGE_CONFIRM");
  expect(installer).toContain("!insertmacro MUI_UNPAGE_INSTFILES");
  expect(installer).not.toContain("Page directory");
  expect(installer).not.toContain("Page license");
});

test("brands MUI with generated Mundus bitmaps, not placeholders", () => {
  // Exact MUI2 define names: a misspelt one is silently ignored and MUI2
  // falls back to its stock win.bmp.
  expect(installer).toContain(
    'MUI_HEADERIMAGE_BITMAP "${STAGE_DIR}\\installer-assets\\header.bmp"',
  );
  expect(installer).toContain(
    'MUI_WELCOMEFINISHPAGE_BITMAP "${STAGE_DIR}\\installer-assets\\welcome.bmp"',
  );
  expect(installer).toContain(
    'MUI_UNWELCOMEFINISHPAGE_BITMAP "${STAGE_DIR}\\installer-assets\\welcome.bmp"',
  );
  expect(installer).toContain("!define MUI_HEADERIMAGE_RIGHT");
  // 2x bitmaps are scaled to the control, so no NOSTRETCH override.
  expect(installer).not.toContain("NOSTRETCH");
});

// Reads a 24-bit BMP's size and the bottom-left pixel as RGB (BMP rows are
// stored bottom-up, pixels as BGR).
function readBmp(name) {
  const bmp = readFileSync(path.join(buildDir, "installer-assets", name));
  const offset = bmp.readUInt32LE(10);
  return {
    width: bmp.readInt32LE(18),
    height: bmp.readInt32LE(22),
    bitsPerPixel: bmp.readUInt16LE(28),
    cornerRgb: [bmp[offset + 2], bmp[offset + 1], bmp[offset]],
  };
}

test("installer bitmaps are 2x, 24-bit and stored as BGR", () => {
  expect(readBmp("header.bmp")).toEqual({
    width: 300,
    height: 114,
    bitsPerPixel: 24,
    cornerRgb: [255, 255, 255],
  });
  expect(readBmp("welcome.bmp")).toEqual({
    width: 328,
    height: 628,
    bitsPerPixel: 24,
    cornerRgb: [22, 20, 30],
  });
});

test("looks native: Unicode, per-monitor DPI, system font, own branding", () => {
  expect(installer).toContain("Unicode true");
  expect(installer).toContain("ManifestDPIAware true");
  expect(installer).toContain("ManifestDPIAwareness PerMonitorV2,System");
  expect(installer).toContain('SetFont "Segoe UI" 9');
  expect(installer).toContain('BrandingText "${APP_NAME} ${VERSION}"');
  expect(installer).toContain("ShowInstDetails hide");
  expect(installer).toContain("ShowUninstDetails hide");
});

test("auto-selects English or Russian from the system UI language", () => {
  expect(installer).toContain('!insertmacro MUI_LANGUAGE "English"');
  expect(installer).toContain('!insertmacro MUI_LANGUAGE "Russian"');
  expect(installer).toContain("GetUserDefaultUILanguage");
  expect(installer).toContain("IntCmp $0 1049");
  expect(installer).toContain("StrCpy $LANGUAGE ${LANG_ENGLISH}");
  expect(installer).toContain("StrCpy $LANGUAGE ${LANG_RUSSIAN}");
});

test("finish page offers a checked 'Launch Mundus' checkbox for the Manager", () => {
  expect(installer).toContain("MUI_FINISHPAGE_RUN");
  expect(installer).toContain("resources\\components\\manager\\${MANAGER_EXE}");
  expect(installer).toContain('MUI_FINISHPAGE_RUN_TEXT "$(FINISHPAGE_RUN_TEXT)"');
  expect(installer).toContain(
    'LangString FINISHPAGE_RUN_TEXT ${LANG_ENGLISH} "Launch ${APP_NAME}"',
  );
  expect(installer).toContain(
    'LangString FINISHPAGE_RUN_TEXT ${LANG_RUSSIAN} "Запустить ${APP_NAME}"',
  );
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

// KOS-306: a single staged-engine call stops the product — graceful
// `--shutdown` first, then the `kill-product-processes` subcommand for the
// leftovers (store apps, 0.9.x/Electron-era names). No taskkill, no
// per-name spawns; the name list lives in runtime/src/installer/processes.rs.
test("stops product processes via the staged engine subcommand", () => {
  expect(installSection).toContain("!insertmacro StopProductProcesses");
  expect(macroSection).toContain("--shutdown");
  expect(macroSection).toContain("kill-product-processes");
  // KOS-306 round 2: the helper runs from $INSTDIR\resources.next — the
  // payload staged inside the install dir, never an exe out of %TEMP%.
  expect(macroSection).toContain('"$INSTDIR\\resources.next\\engine\\mundus-engine.exe"');
  expect(macroSection).toContain("Sleep 500");
  expect(uninstallSection).toContain('mundus-engine.exe" --shutdown');
  expect(uninstallSection).toContain('mundus-engine.exe" kill-product-processes');
  expect(installer).not.toContain("taskkill");
});

// KOS-306: "installer drops an exe into %TEMP% and runs it" is the dropper
// feature this ticket removes — nothing under $PLUGINSDIR may execute, and
// the payload moves by rename, not a second copy.
test("the installer never executes from $PLUGINSDIR and never copies the payload twice", () => {
  for (const line of installer.split("\n")) {
    if (/\b(nsExec|Exec|ExecWait|ExecShell)\b/.test(line)) {
      expect(line).not.toContain("$PLUGINSDIR");
    }
  }
  expect(installer).not.toContain("CopyFiles");
});

test("the payload swap keeps the previous install until the new tree is in place", () => {
  // resources -> resources.old, resources.next -> resources, delete .old —
  // a failed rename-aside aborts, a failed rename-in restores .old.
  expect(installer).toContain('Rename "$INSTDIR\\resources" "$INSTDIR\\resources.old"');
  expect(installer).toContain('Rename "$INSTDIR\\resources.next" "$INSTDIR\\resources"');
  expect(installer).toContain('Rename "$INSTDIR\\resources.old" "$INSTDIR\\resources"');
  expect(installer).toContain("payload_locked:");
  expect(installer).toContain("payload_rollback:");
  // The live tree is renamed aside, never deleted before the new one is in
  // — the uninstaller still owns `RMDir /r` on resources.
  expect(installSection).not.toContain('RMDir /r "$INSTDIR\\resources"');
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

test("always starts the Engine at the end of install; Manager only via finish page", () => {
  expect(installer).toContain("Function StartEngine");
  expect(installSection).toContain("Call StartEngine");
  expect(installer).toContain("post-install --start-engine");
  // The Manager is launched only by the MUI finish-page checkbox, not by a
  // silent Exec in the install section.
  expect(installSection).not.toContain(
    "Exec '\"$INSTDIR\\resources\\components\\manager\\${MANAGER_EXE}\"'",
  );
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
  expect(installSection).toContain('Rename "$INSTDIR\\resources.next" "$INSTDIR\\resources"');
});

// MIGRATION(KOS-267): remove after 2026-11-01.
test("removes the 0.9.x host's per-app Start Menu links without a recursive delete", () => {
  for (const name of ["Agenda", "Memoria", "Dictation", "Ordo"]) {
    expect(installSection).toContain(`Delete "$SMPROGRAMS\\Kosmos\\${name}.lnk"`); // MIGRATION(KOS-267): remove after 2026-11-01
  }
  expect(installSection).toContain('RMDir "$SMPROGRAMS\\Kosmos"'); // MIGRATION(KOS-267): remove after 2026-11-01
  expect(installSection).not.toContain('RMDir /r "$SMPROGRAMS\\Kosmos"'); // MIGRATION(KOS-267): remove after 2026-11-01
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
  // An all-false result must leave an earlier marker alone (installer re-run
  // before the Engine's first start).
  const skipAt = marker.indexOf('"falsefalsefalse" record_done');
  expect(skipAt >= 0).toBeTruthy();
  expect(skipAt).toBeLessThan(marker.indexOf("FileOpen"));
  const recordAt = installSection.indexOf("Call RecordLegacyComponents");
  const kosmosWipeAt = installSection.indexOf('RMDir /r "$LOCALAPPDATA\\Programs\\Kosmos"');
  // KOS-306 round 2: the payload is swapped in by rename, not wiped.
  const resourcesSwapAt = installSection.indexOf('Rename "$INSTDIR\\resources"');
  expect(recordAt >= 0).toBeTruthy();
  expect(kosmosWipeAt >= 0).toBeTruthy();
  expect(resourcesSwapAt >= 0).toBeTruthy();
  expect(recordAt).toBeLessThan(kosmosWipeAt);
  expect(recordAt).toBeLessThan(resourcesSwapAt);
});

test("uninstall removes the store payload dir but never user data", () => {
  expect(uninstallSection).toContain('RMDir /r "${APPS_ROOT}"');
  // User data under the Mundus roots is deliberately kept.
  expect(uninstallSection).not.toContain('RMDir /r "$APPDATA\\Mundus"');
  expect(uninstallSection).not.toContain('RMDir /r "$LOCALAPPDATA\\Mundus"');
});

test("migrates autostart via the staged engine and never using the Desktop VERSION", () => {
  expect(installer).not.toContain("versions\\${VERSION}\\mundus-engine.exe");
  expect(installSection).not.toContain('WriteRegStr HKCU "${RUN_KEY}" "Mundus Engine"');
  expect(installer).toContain("post-install --migrate-autostart");
});

// KOS-306: no script host anywhere in the installer — every former
// PowerShell step is an `install`/`post-install`/`kill-product-processes`
// subcommand of the staged mundus-engine.exe.
test("the installer never spawns PowerShell or a shell script", () => {
  expect(installer.toLowerCase()).not.toContain("powershell");
  expect(installer).not.toContain("ExecutionPolicy");
  expect(installer).not.toContain("-Command");
  expect(installer).not.toContain(".ps1");
});

test("no NSIS single-quoted string contains '' (NSIS has no doubled-quote escape)", () => {
  expect(installer).not.toContain("''");
});

test("migrates autostart and starts the Engine via dedicated functions", () => {
  expect(installer).not.toContain("DetectPreviousInstall");
  expect(installer).toContain("Function SeedOrMigrateAutostart");
  expect(installer).toContain("Function StartEngine");
});

test("ignores legacy electron-updater arguments", () => {
  expect(installer).toContain("Function .onInit");
  expect(installer).toMatch(/--updated|--force-run/);
});
