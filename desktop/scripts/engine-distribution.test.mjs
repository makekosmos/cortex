import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  buildEngineArchive,
  installEngineArchive,
  verifyEngineArchive,
} from "./engine-distribution.mjs";

test("engine archive is independently verifiable and installable", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-engine-distribution-"));
  const release = path.join(root, "release");
  const archive = path.join(root, "Kosmos-Engine-1.2.3.zip");
  for (const name of [
    "kepler-backend.exe",
    "ark-core-rpc.exe",
    "kepler-focus-helper.exe",
    "kepler-focus-svc.exe",
  ]) {
    mkdirSync(release, { recursive: true });
    writeFileSync(path.join(release, name), name);
  }
  const manifest = buildEngineArchive(release, archive, {
    version: "1.2.3",
    url: "https://example.invalid/engine.zip",
  });
  assert.equal(verifyEngineArchive(archive, manifest), true);
  const installed = installEngineArchive(archive, manifest, path.join(root, "engine"));
  assert.equal(
    readFileSync(path.join(installed, "kepler-backend.exe"), "utf8"),
    "kepler-backend.exe",
  );
});

test("engine archive rejects tampering", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-engine-distribution-"));
  const release = path.join(root, "release");
  const archive = path.join(root, "engine.zip");
  mkdirSync(release, { recursive: true });
  for (const name of [
    "kepler-backend.exe",
    "ark-core-rpc.exe",
    "kepler-focus-helper.exe",
    "kepler-focus-svc.exe",
  ])
    writeFileSync(path.join(release, name), "ok");
  const manifest = buildEngineArchive(release, archive, {
    version: "1.2.3",
    url: "https://example.invalid/engine.zip",
  });
  manifest.files[0].sha256 = "0".repeat(64);
  assert.throws(() => verifyEngineArchive(archive, manifest), /engine artifact mismatch/);
});

test("a valid installed engine is preserved", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-engine-distribution-"));
  const release = path.join(root, "release");
  const archive = path.join(root, "engine.zip");
  mkdirSync(release, { recursive: true });
  for (const name of [
    "kepler-backend.exe",
    "ark-core-rpc.exe",
    "kepler-focus-helper.exe",
    "kepler-focus-svc.exe",
  ])
    writeFileSync(path.join(release, name), "new");
  const manifest = buildEngineArchive(release, archive, {
    version: "1.2.3",
    url: "https://example.invalid/engine.zip",
  });
  const engineRoot = path.join(root, "engine");
  const first = installEngineArchive(archive, manifest, engineRoot);
  const second = installEngineArchive(archive, manifest, engineRoot);
  assert.equal(second, first);
  assert.equal(readFileSync(path.join(second, "kepler-backend.exe"), "utf8"), "new");
  writeFileSync(path.join(first, "kepler-backend.exe"), "corrupt");
  const repaired = installEngineArchive(archive, manifest, engineRoot);
  assert.equal(readFileSync(path.join(repaired, "kepler-backend.exe"), "utf8"), "new");
});
