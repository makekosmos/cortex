import { expect, test } from "bun:test";
import path from "node:path";
import {
  AUTOSTART_ARGS,
  launchItemMatchesAutostart,
  legacyAutostartPathCandidates,
  sameArgs,
} from "./settings-autostart";

test("sameArgs requires exact autostart marker args", () => {
  expect(sameArgs(["--autostart"], AUTOSTART_ARGS)).toBe(true);
  expect(sameArgs([], AUTOSTART_ARGS)).toBe(false);
  expect(sameArgs(["--autostart", "--extra"], AUTOSTART_ARGS)).toBe(false);
});

test("launchItemMatchesAutostart matches path args and enabled state", () => {
  const execPath = path.join("C:", "Kosmos", "Kosmos.exe");
  expect(launchItemMatchesAutostart({ path: execPath, args: AUTOSTART_ARGS }, execPath)).toBe(true);
  expect(
    launchItemMatchesAutostart({ path: execPath, args: AUTOSTART_ARGS, enabled: false }, execPath),
  ).toBe(false);
  expect(launchItemMatchesAutostart({ path: execPath, args: [] }, execPath)).toBe(false);
});

test("legacyAutostartPathCandidates includes old install locations once", () => {
  const execPath = path.join("C:", "Users", "me", "Programs", "Kosmos", "Kosmos.exe");
  const localAppData = path.join("C:", "Users", "me", "AppData", "Local");
  const candidates = legacyAutostartPathCandidates(execPath, localAppData);

  expect(candidates).toContain(path.join(path.dirname(execPath), "Kepler.exe"));
  expect(candidates).toContain(path.resolve(localAppData, "Programs", "Kepler", "Kepler.exe"));
  expect(new Set(candidates).size).toBe(candidates.length);
});
