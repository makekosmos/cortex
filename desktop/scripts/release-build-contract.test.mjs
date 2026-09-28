import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { test } from "node:test";

const script = await readFile(path.join(import.meta.dirname, "build-desktop.mjs"), "utf8");
const preflight = await readFile(path.join(import.meta.dirname, "release-preflight.mjs"), "utf8");
const backend = await readFile(path.join(import.meta.dirname, "build-backend.mjs"), "utf8");
const releaseVersions = JSON.parse(
  await readFile(path.join(import.meta.dirname, "..", "release-versions.json"), "utf8"),
);

test("release build verifies locally and leaves publishing to the receipt consumer", () => {
  assert.match(script, /ensureNsis\(/);
  assert.match(script, /createReceipt\(/);
  assert.match(script, /writeReceipt\(/);
  assert.doesNotMatch(script, /publishRelease\(|release create|electron-builder/);
  assert.match(script, /emitProvenance\(/);
  assert.match(script, /Preflight: source, BOM, pins, and ARK artifact/);
  assert.match(script, /--dry-run/);
  assert.doesNotMatch(script, /--clobber/);
  assert.match(script, /process\.env\.KOSMOS_RELEASE_BOM/);
  assert.match(preflight, /release builds require a clean tracked and source worktree/);
  assert.match(preflight, /ARK artifact hash does not match BOM/);
});

test("Engine ships from the same build and version as the GUI (KOS-233)", () => {
  // There is exactly one version source: desktop/release-versions.json.
  // desktop/engine-version.json must not exist, and nothing downloads a
  // separately published Kosmos-Engine-*.zip during a build.
  assert.equal(existsSync(path.join(import.meta.dirname, "..", "engine-version.json")), false);
  assert.ok(releaseVersions.win);
  assert.doesNotMatch(backend, /KOSMOS_ENGINE_RELEASE/);
  assert.doesNotMatch(backend, /consumeEngineArtifacts/);
  assert.doesNotMatch(backend, /releases\/download\/v/);
  assert.match(backend, /KOSMOS_ENGINE_VERSION: productVersion/);
  assert.match(backend, /KOSMOS_ENGINE_SOURCE_COMMIT: sourceCommit/);
  assert.match(backend, /buildEngineArchive\(stageDir, engineArchive/);
  assert.doesNotMatch(script, /copyEngineRelease\(/);
  assert.doesNotMatch(script, /copyEngineManifest\(/);
});
