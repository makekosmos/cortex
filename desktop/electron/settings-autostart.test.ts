import { expect, test } from "bun:test";
import path from "node:path";
import {
  AUTOSTART_ARGS,
  launchItemMatchesAutostart,
  legacyAutostartPathCandidates,
} from "./settings-autostart";

test("launchItemMatchesAutostart matches path args and enabled state", () => {
  const installDir = "CosCast";
  const execPath = path.join("C:", installDir, "CosCast.exe");
  expect(launchItemMatchesAutostart({ path: execPath, args: AUTOSTART_ARGS }, execPath)).toBe(true);
  expect(launchItemMatchesAutostart({ path: execPath }, execPath)).toBe(false);
  expect(
    launchItemMatchesAutostart({ path: execPath, args: AUTOSTART_ARGS, enabled: false }, execPath),
  ).toBe(false);
  expect(launchItemMatchesAutostart({ path: execPath, args: [] }, execPath)).toBe(false);
  expect(
    launchItemMatchesAutostart({ path: execPath, args: ["--autostart", "--extra"] }, execPath),
  ).toBe(false);
});

test("legacyAutostartPathCandidates includes old install locations once", () => {
  const installDir = "CosCast";
  const execPath = path.join("C:", "Users", "me", "Programs", installDir, "CosCast.exe");
  const localAppData = path.join("C:", "Users", "me", "AppData", "Local");
  const candidates = legacyAutostartPathCandidates(execPath, localAppData);

  expect(candidates).toContain(path.join(path.dirname(execPath), "Kosmos.exe"));
  expect(candidates).toContain(path.join(path.dirname(execPath), "Kepler.exe"));
  expect(candidates).toContain(path.resolve(localAppData, "Programs", "Kosmos", "Kosmos.exe"));
  expect(candidates).toContain(path.resolve(localAppData, "Programs", "Kepler", "Kepler.exe"));
  expect(new Set(candidates).size).toBe(candidates.length);
});
