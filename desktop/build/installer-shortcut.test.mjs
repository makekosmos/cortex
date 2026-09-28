import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const buildDir = path.dirname(fileURLToPath(import.meta.url));
const installer = readFileSync(path.join(buildDir, "installer.nsi"), "utf8");

test("Kosmos Start Menu shortcut opens the packaged Manager GPUI", () => {
  const manager = "$INSTDIR\\resources\\components\\manager\\Kosmos Manager.exe";
  expect(installer).toContain('Delete "$SMPROGRAMS\\CosCast.lnk"');
  expect(installer).toContain('Delete "$DESKTOP\\Kosmos.lnk"');
  expect(installer).toContain('Delete "$SMPROGRAMS\\Kosmos.lnk"');
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Kosmos.lnk" "${manager}"`);
});

test("Agenda GPUI component gets a guarded Start Menu shortcut with cleanup", () => {
  const agenda = "$INSTDIR\\resources\\components\\agenda\\Kosmos Agenda.exe";
  expect(installer).toContain(`IfFileExists "${agenda}"`);
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Kosmos Agenda.lnk" "${agenda}"`);
  expect(installer).toContain('Delete "$SMPROGRAMS\\Kosmos Agenda.lnk"');
});

test("Memoria GPUI component gets a guarded Start Menu shortcut with cleanup", () => {
  const memoria = "$INSTDIR\\resources\\components\\memoria\\Kosmos Memoria.exe";
  expect(installer).toContain(`IfFileExists "${memoria}"`);
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Kosmos Memoria.lnk" "${memoria}"`);
  expect(installer).toContain('Delete "$SMPROGRAMS\\Kosmos Memoria.lnk"');
});

test("Dictation GPUI component gets a guarded Start Menu shortcut with cleanup", () => {
  const dictation = "$INSTDIR\\resources\\components\\dictation\\Kosmos Dictation.exe";
  expect(installer).toContain(`IfFileExists "${dictation}"`);
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Kosmos Dictation.lnk" "${dictation}"`);
  expect(installer).toContain('Delete "$SMPROGRAMS\\Kosmos Dictation.lnk"');
});
