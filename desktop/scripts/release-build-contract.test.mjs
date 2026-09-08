import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { test } from "node:test";

const script = await readFile(path.join(import.meta.dirname, "build-desktop.mjs"), "utf8");
const backend = await readFile(path.join(import.meta.dirname, "build-backend.mjs"), "utf8");
const engineVersion = JSON.parse(
  await readFile(path.join(import.meta.dirname, "..", "engine-version.json"), "utf8"),
);

test("release build validates before publishing immutable artifacts", () => {
  assert.match(script, /"--publish", "never"/);
  assert.ok(script.indexOf("emitProvenance(") < script.indexOf("publishRelease(platform"));
  assert.ok(
    script.indexOf("verifyLocalReleaseChannel(outputDir") <
      script.indexOf("publishRelease(platform"),
  );
  assert.ok(
    script.indexOf("emitProvenance(", script.indexOf("async function main")) <
      script.indexOf("runFirstPartyContracts(platform)"),
  );
  assert.ok(
    script.indexOf("runFirstPartyContracts(platform)") <
      script.indexOf("publishRelease(platform", script.indexOf("async function main")),
  );
  assert.doesNotMatch(script, /--clobber/);
  assert.match(script, /process\.env\.KOSMOS_RELEASE_BOM/);
  assert.match(script, /release builds require a clean tracked and source worktree/);
  assert.match(script, /ARK artifact hash does not match BOM/);
  assert.match(script, /app\.name\.endsWith\("\.app"\)/);
});

test("GUI and Engine versions remain independent", () => {
  assert.equal(engineVersion.version, "0.1.3");
  assert.match(backend, /process\.env\.KOSMOS_ENGINE_VERSION \?\? engineVersionConfig\.version/);
  assert.match(backend, /build-engine-installer\.mjs/);
  assert.match(backend, /KOSMOS_ENGINE_REUSE_ARCHIVE/);
  assert.match(backend, /verifyEngineArchive\(engineArchive, engineManifest\)/);
  assert.match(
    backend,
    /releases\/download\/v\$\{engineVersion\}\/Kosmos-Engine-\$\{engineVersion\}\.zip/,
  );
  assert.match(script, /copyEngineRelease\(SHELL_ROOT, engineVersion\)/);
  assert.match(script, /copyEngineManifest\(SHELL_ROOT, engineVersion\)/);
  assert.match(script, /collectArtifacts\(outputDir, platform, version, engineVersion\)/);
  assert.match(script, /rmSync\(path\.join\(SHELL_ROOT, "release", name\)\)/);
});
