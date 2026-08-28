import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const buildDir = path.dirname(fileURLToPath(import.meta.url));
const installer = readFileSync(path.join(buildDir, "installer.nsh"), "utf8");
const manager = "$INSTDIR\\resources\\components\\manager\\Kosmos Manager.exe";

test("CosCast shortcuts open the staged Manager instead of the Shell", () => {
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\CosCast.lnk" "${manager}"`);
  expect(installer).toContain(`CreateShortCut "$DESKTOP\\CosCast.lnk" "${manager}"`);
});
