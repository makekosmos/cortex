import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const buildDir = path.dirname(fileURLToPath(import.meta.url));
const installer = readFileSync(path.join(buildDir, "installer.nsi"), "utf8");
const installSection =
  installer.split('Section "Install"')[1]?.split('Section "Uninstall"')[0] ?? "";

test("Mundus Start Menu shortcut opens the packaged Manager GPUI", () => {
  const manager = "$INSTDIR\\resources\\components\\manager\\${MANAGER_EXE}";
  expect(installer).toContain('!define MANAGER_EXE "Mundus Manager.exe"');
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Mundus.lnk" "${manager}"`);
  expect(installer).toContain('Delete "$DESKTOP\\Mundus.lnk"');
  expect(installer).toContain('Delete "$SMPROGRAMS\\Mundus.lnk"');
});

// The GPUI apps are store-installed — the installer never creates their
// shortcuts, but an upgrade must delete the dead ones: the bundled-era
// "Agenda.lnk"-style names (which point at the wiped components dir) and the
// legacy "Kosmos Agenda.lnk" names (MIGRATION(KOS-267)).
for (const app of ["Agenda", "Memoria", "Dictation"]) {
  test(`${app} gets no new shortcut; both stale names are removed on install`, () => {
    expect(installer).not.toContain(`CreateShortCut "$SMPROGRAMS\\${app}.lnk"`);
    expect(installSection).toContain(`Delete "$SMPROGRAMS\\${app}.lnk"`);
    expect(installSection).toContain(`Delete "$SMPROGRAMS\\Kosmos ${app}.lnk"`); // MIGRATION(KOS-267)
  });
}

// Engine-owned store-app links live in the "$SMPROGRAMS\Mundus\" product
// folder (runtime/src/native_apps/shortcuts.rs) — the flat-name cleanup
// above must never be able to delete them: different location, and the
// uninstaller removes the folder wholesale.
test("engine-owned app links live under a product folder the cleanup cannot hit", () => {
  const deletes = [...installer.matchAll(/Delete "\$SMPROGRAMS\\([^"]+)"/g)].map(
    (match) => match[1],
  );
  for (const target of deletes) {
    expect(target.startsWith("Mundus\\")).toBeFalsy();
  }
  const uninstallSection = installer.split('Section "Uninstall"')[1] ?? "";
  expect(uninstallSection).toContain('RMDir /r "$SMPROGRAMS\\Mundus"');
  // The installer itself never writes into the Engine-owned folder.
  expect(installer).not.toContain('CreateShortCut "$SMPROGRAMS\\Mundus\\');
});
