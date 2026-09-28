import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  buildEngineArchive,
  installEngineArchive,
  resolveInstalledEngine,
  verifyEngineArchive,
} from "./engine-distribution.mjs";

const SOURCE_COMMIT = "a".repeat(40);

test("engine archive is independently verifiable and installable", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-engine-distribution-"));
  const release = path.join(root, "release");
  const archive = path.join(root, "Mundus-Engine-1.2.3.zip");
  for (const name of [
    "mundus-engine.exe",
    "focus-helper.exe",
    "focus-svc.exe",
    "tray.ico",
  ]) {
    mkdirSync(release, { recursive: true });
    writeFileSync(path.join(release, name), name);
  }
  const manifest = buildEngineArchive(release, archive, {
    version: "1.2.3",
    sourceCommit: SOURCE_COMMIT,
  });
  assert.equal(verifyEngineArchive(archive, manifest), true);
  const installed = installEngineArchive(archive, manifest, path.join(root, "engine"));
  assert.equal(
    readFileSync(path.join(installed, "mundus-engine.exe"), "utf8"),
    "mundus-engine.exe",
  );
});

test("buildEngineArchive accepts any semver product version", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-engine-version-"));
  const release = path.join(root, "release");
  const archive = path.join(root, "Mundus-Engine-0.1.0.zip");
  mkdirSync(release, { recursive: true });
  for (const name of [
    "mundus-engine.exe",
    "focus-helper.exe",
    "focus-svc.exe",
    "tray.ico",
  ])
    writeFileSync(path.join(release, name), name);
  const manifest = buildEngineArchive(release, archive, {
    version: "0.1.0",
    sourceCommit: SOURCE_COMMIT,
  });
  assert.equal(manifest.version, "0.1.0");
  assert.equal(verifyEngineArchive(archive, manifest), true);
});

test("engine archive rejects tampering", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-engine-distribution-"));
  const release = path.join(root, "release");
  const archive = path.join(root, "engine.zip");
  mkdirSync(release, { recursive: true });
  for (const name of [
    "mundus-engine.exe",
    "focus-helper.exe",
    "focus-svc.exe",
    "tray.ico",
  ])
    writeFileSync(path.join(release, name), "ok");
  const manifest = buildEngineArchive(release, archive, {
    version: "1.2.3",
    sourceCommit: SOURCE_COMMIT,
  });
  manifest.files[0].sha256 = "0".repeat(64);
  assert.throws(() => verifyEngineArchive(archive, manifest), /engine artifact mismatch/);
});

test("a valid installed engine is preserved", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-engine-distribution-"));
  const release = path.join(root, "release");
  const archive = path.join(root, "engine.zip");
  mkdirSync(release, { recursive: true });
  for (const name of [
    "mundus-engine.exe",
    "focus-helper.exe",
    "focus-svc.exe",
    "tray.ico",
  ])
    writeFileSync(path.join(release, name), "new");
  const manifest = buildEngineArchive(release, archive, {
    version: "1.2.3",
    sourceCommit: SOURCE_COMMIT,
  });
  const engineRoot = path.join(root, "engine");
  const first = installEngineArchive(archive, manifest, engineRoot);
  const second = installEngineArchive(archive, manifest, engineRoot);
  assert.equal(second, first);
  assert.equal(readFileSync(path.join(second, "mundus-engine.exe"), "utf8"), "new");
  writeFileSync(path.join(first, "mundus-engine.exe"), "corrupt");
  const repaired = installEngineArchive(archive, manifest, engineRoot);
  assert.equal(readFileSync(path.join(repaired, "mundus-engine.exe"), "utf8"), "new");
});

test("installed Engine resolution follows current.json and verifies canonical files", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-engine-resolve-"));
  const release = path.join(root, "release");
  const archive = path.join(root, "engine.zip");
  mkdirSync(release, { recursive: true });
  for (const name of [
    "mundus-engine.exe",
    "focus-helper.exe",
    "focus-svc.exe",
    "tray.ico",
  ])
    writeFileSync(path.join(release, name), name);
  const manifest = buildEngineArchive(release, archive, {
    version: "1.2.3",
    sourceCommit: SOURCE_COMMIT,
  });
  const engineRoot = path.join(root, "installed");
  installEngineArchive(archive, manifest, engineRoot);

  const resolved = resolveInstalledEngine(engineRoot);
  assert.equal(resolved.version, "1.2.3");
  assert.equal(resolved.backend, path.join(engineRoot, "versions", "1.2.3", "mundus-engine.exe"));

  writeFileSync(resolved.backend, "tampered");
  assert.throws(() => resolveInstalledEngine(engineRoot), /engine artifact mismatch/);
});
