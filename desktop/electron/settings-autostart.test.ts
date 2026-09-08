import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  AUTOSTART_ARGS,
  engineAutostartPath,
  launchItemMatchesAutostart,
  legacyAutostartPathCandidates,
} from "./settings-autostart";
import { resolveBackendExe } from "./main-backend-bootstrap";

test("launchItemMatchesAutostart matches path args and enabled state", () => {
  const installDir = "Kosmos";
  const execPath = path.join("C:", installDir, "Kosmos.exe");
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

test("engineAutostartPath follows an installed Engine pointer and fails closed", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-engine-"));
  const versionRoot = path.join(root, "versions", "1.2.3");
  mkdirSync(versionRoot, { recursive: true });
  const backend = path.join(versionRoot, "kepler-backend.exe");
  writeFileSync(backend, "fixture");
  writeFileSync(
    path.join(root, "current.json"),
    JSON.stringify({ schema_version: 1, version: "1.2.3" }),
  );
  const previous = process.env.KOSMOS_ENGINE_ROOT;
  process.env.KOSMOS_ENGINE_ROOT = root;
  try {
    expect(engineAutostartPath("C:\\Kosmos\\Kosmos.exe")).toBe(backend);
    writeFileSync(path.join(root, "current.json"), "{}");
    expect(engineAutostartPath("C:\\Kosmos\\Kosmos.exe")).toBe(
      path.join("C:", "Kosmos", "resources", "Kosmos Runtime.exe"),
    );
    for (const version of ["../outside", path.join(root, "absolute")]) {
      writeFileSync(
        path.join(root, "current.json"),
        JSON.stringify({ schema_version: 1, version }),
      );
      expect(engineAutostartPath("C:\\Kosmos\\Kosmos.exe")).toBe(
        path.join("C:", "Kosmos", "resources", "Kosmos Runtime.exe"),
      );
      expect(
        resolveBackendExe({
          dirname: path.join(root, "desktop"),
          env: { KOSMOS_ENGINE_ROOT: root },
          resourcesPath: path.join(root, "resources"),
          platform: "win32",
        }),
      ).toBe(path.join(root, "resources", "kepler-backend.exe"));
    }
  } finally {
    if (previous === undefined) delete process.env.KOSMOS_ENGINE_ROOT;
    else process.env.KOSMOS_ENGINE_ROOT = previous;
  }
});

test("legacyAutostartPathCandidates includes old install locations once", () => {
  const installDir = "Kosmos";
  const execPath = path.join("C:", "Users", "me", "Programs", installDir, "Kosmos.exe");
  const localAppData = path.join("C:", "Users", "me", "AppData", "Local");
  const candidates = legacyAutostartPathCandidates(execPath, localAppData);

  expect(candidates).toContain(path.join(path.dirname(execPath), "Kosmos.exe"));
  expect(candidates).toContain(path.join(path.dirname(execPath), "Kepler.exe"));
  expect(candidates).toContain(path.resolve(localAppData, "Programs", "Kosmos", "Kosmos.exe"));
  expect(candidates).toContain(path.resolve(localAppData, "Programs", "Kepler", "Kepler.exe"));
  expect(new Set(candidates).size).toBe(candidates.length);
});
