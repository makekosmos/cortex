import assert from "node:assert/strict";
import crypto from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  buildEnginePayload,
  ENGINE_FILES,
  validateEngineManifest,
} from "./engine-distribution.mjs";

const SOURCE_COMMIT = "a".repeat(40);

function stage(version = "1.2.3") {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-engine-distribution-"));
  const release = path.join(root, "release");
  mkdirSync(release, { recursive: true });
  for (const name of ENGINE_FILES) writeFileSync(path.join(release, name), name);
  const payload = path.join(root, "payload");
  const manifest = buildEnginePayload(release, payload, {
    version,
    sourceCommit: SOURCE_COMMIT,
  });
  return { root, payload, manifest };
}

test("the payload dir ships every manifest-listed file plus the manifest itself", () => {
  const { payload, manifest } = stage();
  validateEngineManifest(manifest);
  for (const file of manifest.files) {
    const data = readFileSync(path.join(payload, file.name));
    assert.equal(crypto.createHash("sha256").update(data).digest("hex"), file.sha256);
    assert.equal(data.length, file.size);
  }
  const onDisk = JSON.parse(readFileSync(path.join(payload, "engine-manifest.json"), "utf8"));
  assert.equal(onDisk.version, manifest.version);
});

test("buildEnginePayload accepts any semver product version", () => {
  const { manifest } = stage("0.1.0");
  assert.equal(manifest.version, "0.1.0");
});

test("buildEnginePayload rejects non-semver versions and bad source commits", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "mundus-engine-distribution-"));
  const release = path.join(root, "release");
  mkdirSync(release, { recursive: true });
  for (const name of ENGINE_FILES) writeFileSync(path.join(release, name), name);
  assert.throws(
    () =>
      buildEnginePayload(release, path.join(root, "p"), {
        version: "1.2",
        sourceCommit: SOURCE_COMMIT,
      }),
    /semver/,
  );
  assert.throws(
    () =>
      buildEnginePayload(release, path.join(root, "p"), {
        version: "1.2.3",
        sourceCommit: "short",
      }),
    /sourceCommit/,
  );
});

test("validateEngineManifest rejects tampering", () => {
  const { manifest } = stage();
  manifest.files[0].sha256 = "0".repeat(63);
  assert.throws(() => validateEngineManifest(manifest), /invalid engine manifest file/);
});
