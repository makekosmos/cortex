import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { buildEngineArchive } from "./engine-distribution.mjs";

const names = ["mundus-engine.exe", "tray.ico"];
const script = fileURLToPath(new URL("../build/install-engine.ps1", import.meta.url));
const SOURCE_COMMIT = "a".repeat(40);

function fixture(version = "1.2.3") {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-engine-headless-"));
  const release = path.join(root, "release");
  mkdirSync(release, { recursive: true });
  for (const name of names) writeFileSync(path.join(release, name), `fixture:${name}`);
  const archive = path.join(root, "engine.zip");
  const manifest = buildEngineArchive(release, archive, {
    version,
    sourceCommit: SOURCE_COMMIT,
  });
  const manifestPath = path.join(root, "manifest.json");
  writeFileSync(manifestPath, JSON.stringify(manifest));
  return { root, archive, manifestPath, manifest };
}

function runInstall({ archive, manifestPath, root }, extraArgs = []) {
  const args = [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    script,
    "-Archive",
    archive,
    "-Manifest",
    manifestPath,
    "-TargetRoot",
    path.join(root, "installed"),
    ...extraArgs,
  ];
  return spawnSync("powershell", args, { encoding: "utf8" });
}

test("fresh install extracts, verifies, and points current.json at the bundled version", () => {
  const f = fixture();
  const result = runInstall(f);
  assert.equal(result.status, 0, result.stderr);
  const current = JSON.parse(readFileSync(path.join(f.root, "installed", "current.json"), "utf8"));
  assert.equal(current.version, "1.2.3");
  const backend = path.join(f.root, "installed", "versions", "1.2.3", "mundus-engine.exe");
  assert.equal(readFileSync(backend, "utf8"), "fixture:mundus-engine.exe");
});

test("install is idempotent and repairs a corrupted installation from the bundled archive", () => {
  const f = fixture();
  assert.equal(runInstall(f).status, 0);
  const backend = path.join(f.root, "installed", "versions", "1.2.3", "mundus-engine.exe");
  assert.equal(runInstall(f).status, 0);
  assert.equal(readFileSync(backend, "utf8"), "fixture:mundus-engine.exe");
  writeFileSync(backend, "corrupt");
  const repaired = runInstall(f);
  assert.equal(repaired.status, 0, repaired.stderr);
  assert.equal(readFileSync(backend, "utf8"), "fixture:mundus-engine.exe");
});

test("install never downgrades a newer, already-verified Engine", () => {
  const newer = fixture("2.0.0");
  assert.equal(runInstall(newer).status, 0);
  const olderManifestPath = path.join(newer.root, "older-manifest.json");
  const olderArchive = path.join(newer.root, "older.zip");
  const olderRelease = path.join(newer.root, "older-release");
  mkdirSync(olderRelease, { recursive: true });
  for (const name of names) writeFileSync(path.join(olderRelease, name), `older:${name}`);
  const olderManifest = buildEngineArchive(olderRelease, olderArchive, {
    version: "1.0.0",
    sourceCommit: SOURCE_COMMIT,
  });
  writeFileSync(olderManifestPath, JSON.stringify(olderManifest));
  const result = runInstall({
    root: newer.root,
    archive: olderArchive,
    manifestPath: olderManifestPath,
  });
  assert.equal(result.status, 0, result.stderr);
  const current = JSON.parse(
    readFileSync(path.join(newer.root, "installed", "current.json"), "utf8"),
  );
  assert.equal(current.version, "2.0.0", "a lower release must never overwrite a newer pointer");
  assert.equal(
    existsSync(path.join(newer.root, "installed", "versions", "1.0.0")),
    false,
    "an older build is not even staged once a newer Engine is installed",
  );
});

test("a same-version rebuild replaces the installed Engine", () => {
  const f = fixture();
  assert.equal(runInstall(f).status, 0);
  const backend = path.join(f.root, "installed", "versions", "1.2.3", "mundus-engine.exe");
  const rebuild = path.join(f.root, "rebuild");
  mkdirSync(rebuild, { recursive: true });
  for (const name of names) writeFileSync(path.join(rebuild, name), `rebuild:${name}`);
  const archive = path.join(f.root, "rebuild.zip");
  const manifestPath = path.join(f.root, "rebuild-manifest.json");
  writeFileSync(
    manifestPath,
    JSON.stringify(
      buildEngineArchive(rebuild, archive, { version: "1.2.3", sourceCommit: SOURCE_COMMIT }),
    ),
  );
  const result = runInstall({ root: f.root, archive, manifestPath });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(backend, "utf8"), "rebuild:mundus-engine.exe");
  assert.equal(
    JSON.parse(readFileSync(path.join(f.root, "installed", "current.json"), "utf8")).version,
    "1.2.3",
  );
});

test("untrusted engine archive blocks installation", () => {
  const f = fixture();
  writeFileSync(f.archive, "tampered");
  const result = runInstall(f);
  assert.notEqual(result.status, 0);
  assert.equal(existsSync(path.join(f.root, "installed", "current.json")), false);
});

test("unsafe manifest paths and sizes are rejected before installation", () => {
  const f = fixture();
  writeFileSync(
    f.manifestPath,
    JSON.stringify({
      ...f.manifest,
      files: [{ ...f.manifest.files[0], name: "../escape.exe" }],
    }),
  );
  assert.notEqual(runInstall(f).status, 0);
  assert.equal(existsSync(path.join(f.root, "escape.exe")), false);
  writeFileSync(
    f.manifestPath,
    JSON.stringify({
      ...f.manifest,
      files: [{ ...f.manifest.files[0], name: ".." }],
    }),
  );
  assert.notEqual(runInstall(f).status, 0);
  writeFileSync(
    f.manifestPath,
    JSON.stringify({
      ...f.manifest,
      files: [{ ...f.manifest.files[0], size: 1.5 }],
    }),
  );
  assert.notEqual(runInstall(f).status, 0);
});

test("install takes over an existing standalone Mundus Engine registration", () => {
  const f = fixture();
  // A scratch registry key/shortcut, never the real machine state — the
  // script only points at the real "Mundus Engine" registration when these
  // overrides are omitted (see installer.nsi).
  const legacyKey = `HKCU:\\Software\\MundusEngineMigrationTest\\${process.pid}-${Date.now()}`;
  const legacyShortcut = path.join(f.root, "Mundus Engine.lnk");
  writeFileSync(legacyShortcut, "fake shortcut");
  const oldEngineRoot = path.join(f.root, "old-standalone-engine");
  mkdirSync(oldEngineRoot, { recursive: true });
  writeFileSync(path.join(oldEngineRoot, "Uninstall.exe"), "old uninstaller");
  execFileSync("powershell", [
    "-NoProfile",
    "-Command",
    `New-Item -Path '${legacyKey}' -Force | Out-Null;
     New-ItemProperty -LiteralPath '${legacyKey}' -Name DisplayName -Value 'Mundus Engine' -PropertyType String -Force | Out-Null;
     New-ItemProperty -LiteralPath '${legacyKey}' -Name InstallLocation -Value '${oldEngineRoot.replaceAll("\\", "\\\\")}' -PropertyType String -Force | Out-Null;`,
  ]);
  try {
    const result = runInstall(f, [
      "-LegacyRegistryKey",
      legacyKey,
      "-LegacyShortcut",
      legacyShortcut,
    ]);
    assert.equal(result.status, 0, result.stderr);
    const keyGone = spawnSync("powershell", [
      "-NoProfile",
      "-Command",
      `if (Test-Path -LiteralPath '${legacyKey}') { exit 1 } else { exit 0 }`,
    ]);
    assert.equal(keyGone.status, 0, "legacy registry entry must be removed");
    assert.equal(existsSync(legacyShortcut), false, "legacy Start Menu shortcut must be removed");
    assert.equal(
      existsSync(path.join(oldEngineRoot, "Uninstall.exe")),
      false,
      "orphaned standalone uninstaller is cleaned up",
    );
    assert.equal(
      JSON.parse(readFileSync(path.join(f.root, "installed", "current.json"), "utf8")).version,
      "1.2.3",
    );
  } finally {
    spawnSync("powershell", [
      "-NoProfile",
      "-Command",
      `Remove-Item -LiteralPath '${legacyKey}' -Recurse -Force -ErrorAction SilentlyContinue`,
    ]);
    rmSync(f.root, { recursive: true, force: true });
  }
});
