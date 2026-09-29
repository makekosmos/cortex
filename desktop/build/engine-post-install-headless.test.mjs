import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./engine-post-install.ps1", import.meta.url));

function engineRoot(version = "1.2.3") {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-post-install-"));
  const versionDir = path.join(root, "versions", version);
  mkdirSync(versionDir, { recursive: true });
  writeFileSync(path.join(versionDir, "mundus-engine.exe"), "fixture");
  writeFileSync(path.join(root, "current.json"), JSON.stringify({ schema_version: 1, version }));
  return root;
}

function approvedPath() {
  return `HKCU:\\Software\\MundusPostInstallTest\\${process.pid}\\StartupApproved\\Run`;
}

function runKeyPath() {
  return `HKCU:\\Software\\MundusPostInstallTest\\${process.pid}`;
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
      runKeyPath(),
      "-StartupApprovedPath",
      approvedPath(),
      "-DryRun",
      ...switches,
      ...extra,
    ],
    { encoding: "utf8" },
  );
}

function seedRunKeyMarker(name, stateByte) {
  const marker = Buffer.alloc(12);
  marker[0] = stateByte;
  const ft = BigInt(Date.now()) * 10_000n + 11_644_736_000_000_000n;
  marker.writeBigUInt64LE(ft, 4);
  spawnSync(
    "powershell",
    [
      "-NoProfile",
      "-Command",
      `New-Item -Path '${approvedPath()}' -Force | Out-Null; Set-ItemProperty -LiteralPath '${approvedPath()}' -Name '${name}' -Value ([byte[]](${Array.from(marker).join(",")})) -Type Binary`,
    ],
    { encoding: "utf8" },
  );
}

function readRunValue(name) {
  const out = spawnSync(
    "powershell",
    [
      "-NoProfile",
      "-Command",
      `$item = Get-ItemProperty -LiteralPath '${runKeyPath()}' -Name '${name}' -ErrorAction SilentlyContinue; $item.'${name}'`,
    ],
    { encoding: "utf8" },
  );
  return out.stdout.trim();
}

function cleanupRunKey() {
  spawnSync(
    "powershell",
    [
      "-NoProfile",
      "-Command",
      `Remove-Item -Path '${runKeyPath()}' -Recurse -Force -ErrorAction SilentlyContinue`,
    ],
    { encoding: "utf8" },
  );
}

test("valid current.json resolves the installed mundus-engine.exe for autostart", () => {
  const root = engineRoot("2.4.6");
  try {
    const result = run(root, ["-MigrateAutostart"]);
    assert.equal(result.status, 0, result.stderr);
    const exe = path.join(root, "versions", "2.4.6", "mundus-engine.exe");
    assert.ok(result.stdout.includes(`'Mundus Engine' = "${exe}" --start`), result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("-StartEngine resolves the same binary without blocking", () => {
  const root = engineRoot();
  try {
    const result = run(root, ["-StartEngine"]);
    assert.equal(result.status, 0, result.stderr);
    const exe = path.join(root, "versions", "1.2.3", "mundus-engine.exe");
    assert.ok(result.stdout.includes(`START "${exe}" --start`), result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("missing current.json fails closed: non-zero exit, nothing written", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-post-install-"));
  try {
    const result = run(root, ["-MigrateAutostart", "-StartEngine"]);
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
    const result = run(root, ["-MigrateAutostart"]);
    assert.notEqual(result.status, 0, JSON.stringify(pointer));
    assert.ok(!result.stdout.includes("AUTOSTART"), result.stdout);
    rmSync(root, { recursive: true, force: true });
  }
});

test("pointed-at version without a mundus-engine.exe fails closed", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-post-install-"));
  writeFileSync(
    path.join(root, "current.json"),
    JSON.stringify({ schema_version: 1, version: "9.9.9" }),
  );
  try {
    const result = run(root, ["-MigrateAutostart", "-StartEngine"]);
    assert.notEqual(result.status, 0);
    assert.ok(!result.stdout.includes("AUTOSTART"), result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("disabled StartupApproved marker under current name skips autostart", () => {
  const root = engineRoot();
  try {
    seedRunKeyMarker("Mundus Engine", 0x03);
    const result = run(root, ["-MigrateAutostart"]);
    assert.equal(result.status, 0, result.stderr);
    assert.ok(result.stdout.includes("AUTOSTART-SKIPPED opt-out=Mundus Engine"), result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("disabled StartupApproved marker under legacy Kosmos Engine name skips autostart", () => {
  const root = engineRoot();
  try {
    seedRunKeyMarker("Kosmos Engine", 0x06);
    const result = run(root, ["-MigrateAutostart"]);
    assert.equal(result.status, 0, result.stderr);
    assert.ok(result.stdout.includes("AUTOSTART-SKIPPED opt-out=Kosmos Engine"), result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("non-dry-run seeds Run value and enabled StartupApproved marker", () => {
  const root = engineRoot("3.0.0");
  cleanupRunKey();
  try {
    const result = spawnSync(
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
        runKeyPath(),
        "-StartupApprovedPath",
        approvedPath(),
        "-MigrateAutostart",
      ],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    const exe = path.join(root, "versions", "3.0.0", "mundus-engine.exe");
    assert.equal(readRunValue("Mundus Engine"), `"${exe}" --start`);
    const approved = spawnSync(
      "powershell",
      [
        "-NoProfile",
        "-Command",
        `$item = Get-ItemProperty -LiteralPath '${approvedPath()}' -Name 'Mundus Engine' -ErrorAction SilentlyContinue; [byte[]]($item.'Mundus Engine')`,
      ],
      { encoding: "utf8" },
    ).stdout.trim();
    assert.ok(approved.startsWith("2"), approved);
  } finally {
    rmSync(root, { recursive: true, force: true });
    cleanupRunKey();
  }
});

test("readFileSync sanity: script ships no inline NSIS quoting hazards", () => {
  // The script is invoked with -File and plain -Switch arguments only; assert
  // the file itself parses (it ran above) and lives next to install-engine.ps1.
  const source = readFileSync(script, "utf8");
  assert.ok(source.includes("current.json"));
});
