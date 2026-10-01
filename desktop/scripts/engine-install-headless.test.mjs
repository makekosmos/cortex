import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
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

function runInstall({ archive, manifestPath, root }, extraArgs = [], env = {}) {
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
  return spawnSync("powershell", args, {
    encoding: "utf8",
    env: { ...process.env, ...env },
  });
}

// A runnable stand-in for mundus-engine.exe: the fixture above ships a text
// file, which can prove the script survives a failed-to-start prune but can
// never exercise exit-code/output handling. This stub (compiled per test via
// the .NET Framework csc.exe) prints the same JSON outcome line the real
// `prune-versions` mode does and fails on request via PRUNE_STUB_FAIL.
const PRUNE_STUB_CS = `
class P {
  static int Main() {
    if (System.Environment.GetEnvironmentVariable("PRUNE_STUB_FAIL") == "1") {
      System.Console.WriteLine("{\\"ok\\":false,\\"error\\":\\"boom\\"}");
      return 1;
    }
    System.Console.WriteLine("{\\"ok\\":true,\\"report\\":{\\"current\\":\\"1.2.3\\",\\"removed\\":[\\"0.9.0\\",\\"0.9.1\\"],\\"removed_bytes\\":73400320}}");
    return 0;
  }
}`;

function cscExe() {
  const base = path.join(process.env.WINDIR ?? "C:\\Windows", "Microsoft.NET", "Framework64");
  for (const dir of readdirSync(base)) {
    const candidate = path.join(base, dir, "csc.exe");
    if (existsSync(candidate)) return candidate;
  }
  throw new Error(`csc.exe not found under ${base}`);
}

// Same as fixture(), but the staged mundus-engine.exe is the runnable stub.
function fixtureWithStubExe(version = "1.2.3") {
  const f = fixture(version);
  const csPath = path.join(f.root, "prune-stub.cs");
  writeFileSync(csPath, PRUNE_STUB_CS);
  execFileSync(cscExe(), [
    "/nologo",
    "/target:exe",
    `/out:${path.join(f.root, "release", "mundus-engine.exe")}`,
    csPath,
  ]);
  const manifest = buildEngineArchive(path.join(f.root, "release"), f.archive, {
    version,
    sourceCommit: SOURCE_COMMIT,
  });
  writeFileSync(f.manifestPath, JSON.stringify(manifest));
  return f;
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

// KOS-261: after switching current.json the installer asks the just-verified
// Engine to prune old versions/<v> dirs (`<exe> prune-versions`). The
// selection rule itself is tested in Rust (engine_versions::prune); here we
// pin the PowerShell side: a non-zero prune exit reports the error, a
// successful prune logs what was freed, and neither can fail the install.
test("a failed prune reports the engine's error and never fails the install", () => {
  const f = fixtureWithStubExe();
  const result = runInstall(f, [], { PRUNE_STUB_FAIL: "1" });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout + result.stderr, /engine versions prune failed: boom/);
  assert.equal(
    JSON.parse(readFileSync(path.join(f.root, "installed", "current.json"), "utf8")).version,
    "1.2.3",
  );
});

test("a successful prune logs the removed versions and freed bytes", () => {
  const f = fixtureWithStubExe();
  const result = runInstall(f);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /PRUNED old engine versions: 0\.9\.0, 0\.9\.1 \(freed 70 MB\)/);
});

test("a prune that cannot even start is still only a warning", () => {
  const f = fixture();
  // The fixture exe is a plain text file: Start-Process throws, the catch
  // warns, and the install still completes with the pointer switched.
  const result = runInstall(f);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout + result.stderr, /engine versions prune failed/);
  assert.equal(
    JSON.parse(readFileSync(path.join(f.root, "installed", "current.json"), "utf8")).version,
    "1.2.3",
  );
  assert.deepEqual(readdirSync(path.join(f.root, "installed", "versions")), ["1.2.3"]);
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
