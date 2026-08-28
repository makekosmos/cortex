import { expect, test } from "bun:test";
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
const shell = "$INSTDIR\\CosCast.exe";

test("CosCast shortcuts open the packaged Shell", () => {
  expect(installer).toContain(`CreateShortCut "$SMPROGRAMS\\CosCast.lnk" "${shell}"`);
  expect(installer).toContain(`CreateShortCut "$DESKTOP\\CosCast.lnk" "${shell}"`);
});

test("rename keeps the legacy app identity and shell IPC namespace", () => {
  expect(desktopPackage.build.appId).toBe("com.kazui.kosmos");
  expect(electronMain).toContain('"com.kosmos.shell"');
});
