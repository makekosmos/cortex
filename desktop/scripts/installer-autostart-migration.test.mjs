import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";

const LEGACY_AUTOSTART_NAMES = [
  "Mundus",
  "com.kazui.kepler",
  "Mundus",
  "KeplerKosmos",
  "KosmosKepler",
];

const installer = readFileSync(path.join(import.meta.dirname, "../build/installer.nsi"), "utf8");
const runKey = "Mundus Engine";

test("installer autostart owns the Engine, not the shell, and cleans legacy names", () => {
  // Regression: 2026-08-01. Updates must not leave duplicate startup owners.
  expect(installer).toContain(`WriteRegStr HKCU "${"${RUN_KEY}"}" "${runKey}"`);
  // The installer registers the Engine (mundus-engine.exe), never a shell exe.
  expect(installer).toContain(`mundus-engine.exe" --start`);
  expect(installer).not.toContain(`"${runKey}" "$INSTDIR\\Mundus.exe"`);

  for (const name of LEGACY_AUTOSTART_NAMES) {
    expect(installer).toContain(`DeleteRegValue HKCU "${"${RUN_KEY}"}" "${name}"`);
  }
});
