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
  "tray.ico",
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
    url: "https://github.com/makekosmos/desktop/releases/download/v1.2.3/engine.zip",
  });
  const manifestPath = path.join(root, "manifest.json");
  writeFileSync(manifestPath, JSON.stringify(manifest));
  return { root, archive, manifestPath, manifest };
}

function runInstall({ archive, manifestPath, root, url }) {
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
  ];
  if (url) args.push("-Url", url);
  return spawnSync("powershell", args, { encoding: "utf8" });
}

function runInstallWithFakeRedownload({ archive, manifestPath, root, sourceArchive }) {
  const quote = (value) => `'${value.replaceAll("'", "''")}'`;
  const wrapper = path.join(root, "redownload.ps1");
  writeFileSync(
    wrapper,
    `function Invoke-WebRequest {
  param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing)
  if ($Uri -ne 'https://github.com/makekosmos/desktop/releases/download/v1.2.3/engine.zip') { throw "unexpected download URL: $Uri" }
  Copy-Item -LiteralPath ${quote(sourceArchive)} -Destination $OutFile -Force
}
& ${quote(script)} -Archive ${quote(archive)} -Manifest ${quote(manifestPath)} -TargetRoot ${quote(path.join(root, "installed"))} -Url 'https://github.com/makekosmos/desktop/releases/download/v1.2.3/engine.zip'
exit $LASTEXITCODE
`,
    "utf8",
  );
  return spawnSync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", wrapper], {
    encoding: "utf8",
  });
}

test("stale archive is replaced by a trusted redownload", () => {
  const f = fixture();
  const sourceArchive = path.join(f.root, "trusted-engine.zip");
  cpSync(f.archive, sourceArchive);
  writeFileSync(f.archive, "stale archive from previous update");
  const result = runInstallWithFakeRedownload({ ...f, sourceArchive });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(f.archive).equals(readFileSync(sourceArchive)), true);
  assert.equal(
    JSON.parse(readFileSync(path.join(f.root, "installed", "current.json"), "utf8")).version,
    "1.2.3",
  );
});

test("corrupt redownload preserves the existing archive", () => {
  const f = fixture();
  const first = runInstall(f);
  assert.equal(first.status, 0, first.stderr);
  const staleArchive = Buffer.from("stale archive from previous update", "utf8");
  writeFileSync(f.archive, staleArchive);
  writeFileSync(
    path.join(f.root, "installed", "current.json"),
    JSON.stringify({ schema_version: 1, version: "0.0.1" }),
  );
  const corruptDownload = path.join(f.root, "corrupt-engine.zip");
  writeFileSync(corruptDownload, "corrupt trusted response");
  const result = runInstallWithFakeRedownload({
    ...f,
    sourceArchive: corruptDownload,
  });
  assert.notEqual(result.status, 0);
  assert.deepEqual(readFileSync(f.archive), staleArchive);
  assert.equal(
    JSON.parse(readFileSync(path.join(f.root, "installed", "current.json"), "utf8")).version,
    "0.0.1",
  );
});

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
  const result = runInstall({ ...f, url: "https://evil.example/engine.zip" });
  assert.notEqual(result.status, 0);
  assert.equal(existsSync(path.join(f.root, "installed")), false);
});

test("untrusted engine publisher metadata blocks GUI dependency install", () => {
  const f = fixture();
  writeFileSync(
    f.manifestPath,
    JSON.stringify({ ...f.manifest, url: "https://evil.example/engine.zip" }),
  );
  const result = runInstall(f);
  assert.notEqual(result.status, 0);
  assert.equal(existsSync(path.join(f.root, "installed")), false);
});

test("unsafe manifest paths and sizes are rejected before installation", () => {
  const f = fixture();
  writeFileSync(
    f.manifestPath,
    JSON.stringify({ ...f.manifest, files: [{ ...f.manifest.files[0], name: "../escape.exe" }] }),
  );
  const result = runInstall(f);
  assert.notEqual(result.status, 0);
  assert.equal(existsSync(path.join(f.root, "escape.exe")), false);
  writeFileSync(
    f.manifestPath,
    JSON.stringify({ ...f.manifest, files: [{ ...f.manifest.files[0], name: ".." }] }),
  );
  assert.notEqual(runInstall(f).status, 0);
  writeFileSync(
    f.manifestPath,
    JSON.stringify({ ...f.manifest, files: [{ ...f.manifest.files[0], size: 1.5 }] }),
  );
  assert.notEqual(runInstall(f).status, 0);
});
