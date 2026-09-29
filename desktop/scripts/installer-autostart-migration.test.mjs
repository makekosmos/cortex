import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";

const installer = readFileSync(path.join(import.meta.dirname, "../build/installer.nsi"), "utf8");
const installSection =
  installer.split('Section "Install"')[1]?.split('Section "Uninstall"')[0] ?? "";

const LEGACY_AUTOSTART_NAMES = [
  "com.kazui.kosmos",
  "electron.app.Kosmos",
  "Kosmos",
  "Kosmos Engine",
  "com.kazui.kepler",
  "Kepler",
  "KeplerKosmos",
  "KosmosKepler",
];

test("installer autostart delegates to engine-post-install.ps1 and never writes Run inline", () => {
  // The Engine binary path is resolved from current.json; the Run value is
  // written by the shipped PowerShell script, not by NSIS inline registry
  // commands, so there is no version string interpolation hazard.
  expect(installer).not.toContain('WriteRegStr HKCU "${RUN_KEY}" "Mundus Engine"');
  expect(installer).toContain(
    '-File "$INSTDIR\\resources\\engine-post-install.ps1" -MigrateAutostart',
  );
  expect(installer).toContain("Function SeedOrMigrateAutostart");
});

test("installer deletes every legacy autostart Run value unconditionally", () => {
  expect(installer).toContain("!insertmacro DeleteOldRunValues");
  for (const name of LEGACY_AUTOSTART_NAMES) {
    expect(installer).toContain(`DeleteRegValue HKCU "${"${RUN_KEY}"}" "${name}"`);
  }
  // The deletion happens inside the Install section before any new autostart
  // is seeded, so a stale legacy value cannot resurrect an old binary.
  const deleteAt = installSection.indexOf("!insertmacro DeleteOldRunValues");
  const seedAt = installSection.indexOf("SeedOrMigrateAutostart");
  expect(deleteAt).toBeGreaterThan(-1);
  expect(seedAt).toBeGreaterThan(-1);
  expect(deleteAt).toBeLessThan(seedAt);
});
