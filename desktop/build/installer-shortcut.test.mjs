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

test("Agenda GPUI component gets a guarded Start Menu shortcut with cleanup", () => {
  const agenda = "$INSTDIR\\resources\\components\\agenda\\Agenda.exe";
  expect(installer).toContain(`IfFileExists "${agenda}"`);
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Agenda.lnk" "${agenda}"`);
  expect(installer).toContain('Delete "$SMPROGRAMS\\Agenda.lnk"');
});

test("Memoria GPUI component gets a guarded Start Menu shortcut with cleanup", () => {
  const memoria = "$INSTDIR\\resources\\components\\memoria\\Memoria.exe";
  expect(installer).toContain(`IfFileExists "${memoria}"`);
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Memoria.lnk" "${memoria}"`);
  expect(installer).toContain('Delete "$SMPROGRAMS\\Memoria.lnk"');
});

test("Dictation GPUI component gets a guarded Start Menu shortcut with cleanup", () => {
  const dictation = "$INSTDIR\\resources\\components\\dictation\\Dictation.exe";
  expect(installer).toContain(`IfFileExists "${dictation}"`);
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Dictation.lnk" "${dictation}"`);
  expect(installer).toContain('Delete "$SMPROGRAMS\\Dictation.lnk"');
});
