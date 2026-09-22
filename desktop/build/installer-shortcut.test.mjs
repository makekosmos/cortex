import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const buildDir = path.dirname(fileURLToPath(import.meta.url));
const installer = readFileSync(path.join(buildDir, "installer.nsh"), "utf8");
const desktopPackage = JSON.parse(readFileSync(path.join(buildDir, "..", "package.json"), "utf8"));
const electronMain = readFileSync(
  path.join(buildDir, "..", "electron", "main-launcher.ts"),
  "utf8",
);
const shell = "$INSTDIR\\Kosmos.exe";

test("Kosmos shortcuts open the packaged Shell", () => {
  expect(installer).toContain('Delete "$SMPROGRAMS\\CosCast.lnk"');
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Kosmos.lnk" "${shell}"`);
  expect(installer).toContain(`CreateShortCut "$DESKTOP\\Kosmos.lnk" "${shell}"`);
});

test("Agenda GPUI component gets a guarded Start Menu shortcut with cleanup", () => {
  const agenda = "$INSTDIR\\resources\\components\\agenda\\Kosmos Agenda.exe";
  expect(installer).toContain(`IfFileExists "${agenda}"`);
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\Kosmos Agenda.lnk" "${agenda}"`);
  expect(installer).toContain('Delete "$SMPROGRAMS\\Kosmos Agenda.lnk"');
});

test("rename keeps the legacy app identity and shell IPC namespace", () => {
  expect(desktopPackage.build.appId).toBe("com.kazui.kosmos");
  expect(electronMain).toContain('"com.kosmos.shell"');
});
