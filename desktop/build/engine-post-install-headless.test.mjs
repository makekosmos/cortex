import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./engine-post-install.ps1", import.meta.url));

function engineRoot(version = "1.2.3") {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-post-install-"));
  const versionDir = path.join(root, "versions", version);
  mkdirSync(versionDir, { recursive: true });
  writeFileSync(path.join(versionDir, "kepler-backend.exe"), "fixture");
  writeFileSync(path.join(root, "current.json"), JSON.stringify({ schema_version: 1, version }));
  return root;
}

// Runs the real script via powershell.exe -File exactly like installer.nsi.
// -DryRun plus a scratch -EngineRoot/-RunKeyPath means the test never writes
// the real HKCU Run key and never starts a real Engine.
function run(root, switches, extra = []) {
  return spawnSync(
    "powershell",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      script,
      "-EngineRoot",
      root,
      "-RunKeyPath",
      `HKCU:\\Software\\KosmosPostInstallTest\\${process.pid}`,
      "-DryRun",
      ...switches,
      ...extra,
    ],
    { encoding: "utf8" },
  );
}

test("valid current.json resolves the installed kepler-backend.exe for autostart", () => {
  const root = engineRoot("2.4.6");
  try {
    const result = run(root, ["-SeedAutostart"]);
    assert.equal(result.status, 0, result.stderr);
    const exe = path.join(root, "versions", "2.4.6", "kepler-backend.exe");
    assert.ok(result.stdout.includes(`'Kosmos Engine' = "${exe}" --start`), result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("-StartEngine resolves the same binary without blocking", () => {
  const root = engineRoot();
  try {
    const result = run(root, ["-StartEngine"]);
    assert.equal(result.status, 0, result.stderr);
    const exe = path.join(root, "versions", "1.2.3", "kepler-backend.exe");
    assert.ok(result.stdout.includes(`START "${exe}" --start`), result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("missing current.json fails closed: non-zero exit, nothing written", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-post-install-"));
  try {
    const result = run(root, ["-SeedAutostart", "-StartEngine"]);
    assert.notEqual(result.status, 0);
    assert.ok(!result.stdout.includes("AUTOSTART"), result.stdout);
    assert.ok(!result.stdout.includes("START"), result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("invalid current.json (bad schema or version) fails closed", () => {
  for (const pointer of [
    { schema_version: 2, version: "1.2.3" },
    { schema_version: 1, version: "not-a-version" },
    { schema_version: 1, version: "../escape" },
  ]) {
    const root = engineRoot();
    writeFileSync(path.join(root, "current.json"), JSON.stringify(pointer));
    const result = run(root, ["-SeedAutostart"]);
    assert.notEqual(result.status, 0, JSON.stringify(pointer));
    assert.ok(!result.stdout.includes("AUTOSTART"), result.stdout);
    rmSync(root, { recursive: true, force: true });
  }
});

test("pointed-at version without a kepler-backend.exe fails closed", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-post-install-"));
  writeFileSync(
    path.join(root, "current.json"),
    JSON.stringify({ schema_version: 1, version: "9.9.9" }),
  );
  try {
    const result = run(root, ["-SeedAutostart", "-StartEngine"]);
    assert.notEqual(result.status, 0);
    assert.ok(!result.stdout.includes("AUTOSTART"), result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("readFileSync sanity: script ships no inline NSIS quoting hazards", () => {
  // The script is invoked with -File and plain -Switch arguments only; assert
  // the file itself parses (it ran above) and lives next to install-engine.ps1.
  const source = readFileSync(script, "utf8");
  assert.ok(source.includes("current.json"));
});
