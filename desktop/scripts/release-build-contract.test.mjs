import assert from "node:assert/strict";
import { existsSync, readdirSync } from "node:fs";
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
  assert.match(script, /Preflight: source, BOM, and pins/);
  assert.match(script, /--dry-run/);
  assert.doesNotMatch(script, /--clobber/);
  assert.match(script, /deriveReleaseBom\(/);
  assert.doesNotMatch(script, /--bom|RELEASE_BOM"|loadReleaseBom/);
  assert.match(preflight, /release builds require a clean tracked and source worktree/);
});

test("makensis reads installer.nsi as UTF-8, so its Russian strings survive any code page", () => {
  assert.match(script, /"\/INPUTCHARSET",\s*"UTF8",/);
});

test("Engine ships from the same build and version as the GUI (KOS-233)", () => {
  // There is exactly one version source: desktop/release-versions.json.
  // desktop/engine-version.json must not exist, and nothing downloads a
  // separately published Mundus-Engine-*.zip during a build.
  assert.equal(existsSync(path.join(import.meta.dirname, "..", "engine-version.json")), false);
  assert.ok(releaseVersions.win);
  assert.doesNotMatch(backend, /MUNDUS_ENGINE_RELEASE/);
  assert.doesNotMatch(backend, /consumeEngineArtifacts/);
  assert.doesNotMatch(backend, /releases\/download\/v/);
  assert.match(backend, /MUNDUS_PRODUCT_VERSION: productVersion/);
  assert.match(backend, /MUNDUS_ENGINE_SOURCE_COMMIT: sourceCommit/);
  assert.match(backend, /buildEngineArchive\(stageDir, engineArchive/);
  assert.doesNotMatch(script, /copyEngineRelease\(/);
  assert.doesNotMatch(script, /copyEngineManifest\(/);
});

// KOS-278: the Engine updater read MUNDUS_PRODUCT_VERSION while the build set
// only MUNDUS_ENGINE_VERSION, so released 0.10.0 reported the crate's 0.1.0 and
// offered 0.10.0 as an update forever. Every compile-time version variable a
// shipped binary reads must be the one both builds inject.
test("shipped binaries read the product version only from the variable the builds set", async () => {
  const repo = path.join(import.meta.dirname, "..", "..");
  const components = await readFile(
    path.join(import.meta.dirname, "build-package-components.mjs"),
    "utf8",
  );
  assert.match(backend, /MUNDUS_PRODUCT_VERSION: productVersion/);
  assert.match(components, /MUNDUS_PRODUCT_VERSION: getVersion\("win"\)/);
  const readers = [];
  for (const dir of ["runtime/src", "manager-gpui/src", "core/crates"]) {
    for (const entry of readdirSync(path.join(repo, dir), { recursive: true })) {
      if (!entry.endsWith(".rs")) continue;
      const source = await readFile(path.join(repo, dir, entry), "utf8");
      for (const [, name] of source.matchAll(/option_env!\("([A-Z0-9_]*VERSION[A-Z0-9_]*)"\)/g))
        readers.push(`${dir}/${entry.replaceAll("\\", "/")}: ${name}`);
    }
  }
  assert.ok(readers.length > 0, "expected the product version to be read somewhere");
  for (const reader of readers) assert.match(reader, /: MUNDUS_PRODUCT_VERSION$/, reader);
});
