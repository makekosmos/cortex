import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";
const LEGACY_AUTOSTART_NAMES = [
  "Kosmos",
  "com.kazui.kepler",
  "Kepler",
  "KeplerKosmos",
  "KosmosKepler",
];

const installer = readFileSync(path.join(import.meta.dir, "../build/installer.nsh"), "utf8");
const installBody = installer.slice(
  installer.indexOf("!macro customInstall"),
  installer.indexOf("!macroend", installer.indexOf("!macro customInstall")),
);
const freshBranch = installBody.slice(0, installBody.indexOf("${else}"));
const updateBranch = installBody.slice(installBody.indexOf("${else}"));
const runKey = "Kosmos Engine";
const runCommand = '"$INSTDIR\\resources\\Kosmos Runtime.exe" --start';

function migrate(values) {
  const next = { ...values };
  const enabled = LEGACY_AUTOSTART_NAMES.some((name) => next[name]);
  if (enabled) next[runKey] = runCommand;
  for (const name of LEGACY_AUTOSTART_NAMES) delete next[name];
  return next;
}

test("installer migration tracks every authoritative legacy name and is idempotent", () => {
  // Regression: 2026-08-01. Updates must not leave duplicate startup owners.
  const reads = [...updateBranch.matchAll(/ReadRegStr \$0 .*?Run" "([^"]+)"/g)].map((m) => m[1]);
  const deletes = [...updateBranch.matchAll(/DeleteRegValue .*?Run" "([^"]+)"/g)].map((m) => m[1]);
  expect(reads).toEqual(LEGACY_AUTOSTART_NAMES);
  expect(deletes).toEqual(LEGACY_AUTOSTART_NAMES);
  // The engine is installed independently; GUI setup must not create an
  // autostart owner that points into the GUI install directory.
  expect(installBody).not.toContain("WriteRegStr HKCU");
  expect(freshBranch).toContain("${ifNot} ${isUpdated}");
  expect(freshBranch).not.toContain(runCommand);
  for (const name of LEGACY_AUTOSTART_NAMES) {
    expect(freshBranch).toContain(
      `DeleteRegValue HKCU "Software\\Microsoft\\Windows\\CurrentVersion\\Run" "${name}"`,
    );
  }

  const cases = [
    {},
    ...LEGACY_AUTOSTART_NAMES.map((name) => ({ [name]: "legacy" })),
    { Kosmos: "legacy", KeplerKosmos: "legacy" },
    Object.fromEntries(LEGACY_AUTOSTART_NAMES.map((name) => [name, "legacy"])),
  ];
  for (const values of cases) {
    const migrated = migrate(values);
    const enabled = LEGACY_AUTOSTART_NAMES.some((name) => values[name]);
    expect(migrated[runKey]).toBe(enabled ? runCommand : undefined);
    for (const name of LEGACY_AUTOSTART_NAMES) expect(migrated[name]).toBeUndefined();
    expect(migrate(migrated)).toEqual(migrated);
  }
});
