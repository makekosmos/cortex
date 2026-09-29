import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const buildDir = path.dirname(fileURLToPath(import.meta.url));
const installer = readFileSync(path.join(buildDir, "installer.nsi"), "utf8");

test("Mundus Start Menu shortcut opens the packaged Manager GPUI", () => {
  const manager = "$INSTDIR\\resources\\components\\manager\\${MANAGER_EXE}";
  expect(installer).toContain('!define MANAGER_EXE "Mundus Manager.exe"');
  expect(installer).toContain('Delete "$SMPROGRAMS\\CosCast.lnk"'); // MIGRATION(KOS-267)
  expect(installer).toContain('Delete "$DESKTOP\\Mundus.lnk"');
  expect(installer).toContain('Delete "$SMPROGRAMS\\Mundus.lnk"');
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Mundus.lnk" "${manager}"`);
});

// KOS-265: the GPUI apps are store-installed native apps — the installer must
// never create component shortcuts, but it cleans up stale ones left by a
// bundled 0.9.x install (legacy Kosmos shortcut names — MIGRATION(KOS-267)).
for (const app of ["Agenda", "Memoria", "Dictation"]) {
  test(`${app} has no installer-created shortcut; stale shortcut is removed`, () => {
    expect(installer).not.toContain(`CreateShortCut "$SMPROGRAMS\\${app}.lnk"`);
    expect(installer).not.toContain(`components\\${app.toLowerCase()}\\`);
    // The stale 0.9.x-era shortcut name stays removable.
    expect(installer).toContain(`Delete "$SMPROGRAMS\\Kosmos ${app}.lnk"`); // MIGRATION(KOS-267)
  });
}
