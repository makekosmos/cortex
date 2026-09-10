import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { buildEngineArchive } from "./engine-distribution.mjs";
import { consumeEngineArtifacts, trustedEngineUrls } from "./engine-consumer.mjs";

const hash = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
const names = [
  "kepler-backend.exe",
  "ark-core-rpc.exe",
  "kepler-focus-helper.exe",
  "kepler-focus-svc.exe",
];

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-engine-consumer-"));
  const release = path.join(root, "release");
  fs.mkdirSync(release);
  for (const name of names) fs.writeFileSync(path.join(release, name), `generated:${name}`);
  const version = "0.1.3";
  const urls = trustedEngineUrls(version);
  const archivePath = path.join(root, "source.zip");
  const manifest = buildEngineArchive(release, archivePath, { version, url: urls.archive });
  const installer = Buffer.from("standalone installer fixture");
  const manifestWithInstaller = {
    ...manifest,
    installer_url: urls.installer,
    installer_sha256: hash(installer),
    installer_size: installer.length,
  };
  const manifestPath = path.join(root, "manifest.json");
  const installerPath = path.join(root, "source.exe");
  fs.writeFileSync(manifestPath, JSON.stringify(manifestWithInstaller));
  fs.writeFileSync(installerPath, installer);
  return { root, version, urls, archivePath, installerPath, manifestPath, manifestWithInstaller };
}

test("Desktop consumes a verified generated Engine fixture without compiling NSIS", async () => {
  const f = fixture();
  const targetDir = path.join(f.root, "engine.next");
  await consumeEngineArtifacts({
    version: f.version,
    targetDir,
    reuse: { manifest: f.manifestPath, archive: f.archivePath, installer: f.installerPath },
  });
  assert.deepEqual(
    fs.readFileSync(path.join(targetDir, "Kosmos-Engine.zip")),
    fs.readFileSync(f.archivePath),
  );
  assert.deepEqual(
    fs.readFileSync(path.join(targetDir, "Kosmos-Engine-Setup-0.1.3.exe")),
    fs.readFileSync(f.installerPath),
  );
  const tampered = path.join(f.root, "tampered.exe");
  fs.writeFileSync(tampered, "tampered");
  await assert.rejects(
    consumeEngineArtifacts({
      version: f.version,
      targetDir: path.join(f.root, "rejected"),
      reuse: { manifest: f.manifestPath, archive: f.archivePath, installer: tampered },
    }),
    /installer hash or size mismatch/,
  );
});

test("Desktop auto-downloads only the pinned Engine URLs and verifies hashes", async () => {
  const f = fixture();
  const requests = [];
  const payloads = new Map([
    [f.urls.manifest, Buffer.from(JSON.stringify(f.manifestWithInstaller))],
    [f.urls.archive, fs.readFileSync(f.archivePath)],
    [f.urls.installer, fs.readFileSync(f.installerPath)],
  ]);
  const targetDir = path.join(f.root, "downloaded");
  await consumeEngineArtifacts({
    version: f.version,
    targetDir,
    fetchImpl: async (url) => {
      requests.push(url);
      return { ok: true, arrayBuffer: async () => payloads.get(url) };
    },
  });
  assert.deepEqual(requests, [f.urls.manifest, f.urls.archive, f.urls.installer]);
  assert.equal(fs.existsSync(path.join(targetDir, "engine-manifest.json")), true);
});
