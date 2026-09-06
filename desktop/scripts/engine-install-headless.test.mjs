import assert from "node:assert/strict";
import { cpSync, existsSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { buildEngineArchive } from "./engine-distribution.mjs";

const names = [
  "kepler-backend.exe",
  "ark-core-rpc.exe",
  "kepler-focus-helper.exe",
  "kepler-focus-svc.exe",
];
const script = fileURLToPath(new URL("../build/install-engine.ps1", import.meta.url));

function fixture() {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-engine-headless-"));
  const release = path.join(root, "release");
  mkdirSync(release, { recursive: true });
  for (const name of names) writeFileSync(path.join(release, name), `fixture:${name}`);
  const archive = path.join(root, "engine.zip");
  const manifest = buildEngineArchive(release, archive, {
    version: "1.2.3",
    url: "https://example.invalid/engine.zip",
  });
  const manifestPath = path.join(root, "manifest.json");
  writeFileSync(manifestPath, JSON.stringify(manifest));
  return { root, archive, manifestPath, manifest };
}

function runInstall({ archive, manifestPath, root }) {
  return spawnSync(
    "powershell",
    [
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
    ],
    { encoding: "utf8" },
  );
}

test("GUI dependency installs absent engine and reuses present engine", () => {
  const f = fixture();
  const first = runInstall(f);
  assert.equal(first.status, 0, first.stderr);
  const current = JSON.parse(readFileSync(path.join(f.root, "installed", "current.json"), "utf8"));
  const backend = path.join(f.root, "installed", "versions", current.version, "kepler-backend.exe");
  const second = runInstall(f);
  assert.equal(second.status, 0, second.stderr);
  assert.equal(readFileSync(backend, "utf8"), "fixture:kepler-backend.exe");
  cpSync(
    path.join(f.root, "installed", "versions", current.version),
    path.join(f.root, "installed", "versions", "0.0.1"),
    { recursive: true },
  );
  writeFileSync(
    path.join(f.root, "installed", "current.json"),
    JSON.stringify({ schema_version: 1, version: "0.0.1" }),
  );
  const versionRepaired = runInstall(f);
  assert.equal(versionRepaired.status, 0, versionRepaired.stderr);
  assert.equal(
    JSON.parse(readFileSync(path.join(f.root, "installed", "current.json"), "utf8")).version,
    "1.2.3",
  );
  writeFileSync(backend, "corrupt");
  const repaired = runInstall(f);
  assert.equal(repaired.status, 0, repaired.stderr);
  assert.equal(readFileSync(backend, "utf8"), "fixture:kepler-backend.exe");
});

test("untrusted engine archive blocks GUI dependency install", () => {
  const f = fixture();
  writeFileSync(f.archive, "tampered");
  const result = runInstall(f);
  assert.notEqual(result.status, 0);
  assert.equal(existsSync(path.join(f.root, "installed")), false);
});
