import assert from "node:assert/strict";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  effectiveCargoTargetDir,
  acquireBuildLock,
  cleanBuildIntermediates,
  RUNTIME_BINARIES,
  stageRuntimeBinaries,
} from "./runtime-staging.mjs";
import { ARK_CORE_REPOSITORY, ARK_CORE_REVISION } from "./ark-core-rpc.mjs";

const shellRoot = fileURLToPath(new URL("..", import.meta.url));
const cortexRoot = path.resolve(shellRoot, "..");

test("non-default Cargo runtime is the one mapped into the Windows package", () => {
  assert.deepEqual(RUNTIME_BINARIES, [
    "kepler-backend",
    "ark-core-rpc",
    "kepler-focus-helper",
    "kepler-focus-svc",
  ]);
  assert.match(ARK_CORE_REPOSITORY, /^https:\/\/github\.com\/makekosmos\/core\.git$/);
  assert.equal(ARK_CORE_REVISION, "80d74cdc711d7601db8a351ee1d26d5c1bdfe34f");

  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-runtime-stage-"));
  try {
    const configured = path.join(root, "alternate-cargo-target");
    const releaseDir = path.join(configured, "release");
    const stageDir = path.join(root, "stage");
    mkdirSync(releaseDir, { recursive: true });
    for (const binary of RUNTIME_BINARIES) {
      writeFileSync(path.join(releaseDir, `${binary}.exe`), `fresh:${binary}`);
    }

    assert.equal(effectiveCargoTargetDir(shellRoot, cortexRoot, configured), configured);
    stageRuntimeBinaries(releaseDir, stageDir, "win32");

    const packageJson = JSON.parse(readFileSync(path.join(shellRoot, "package.json"), "utf8"));
    const backendBuild = readFileSync(path.join(shellRoot, "scripts", "build-backend.mjs"), "utf8");
    const engineReleaseBuild = readFileSync(
      path.join(shellRoot, "scripts", "build-engine-release.mjs"),
      "utf8",
    );
    assert.match(backendBuild, /let engineVersion = null/);
    assert.match(backendBuild, /if \(engineVersion\) console\.log/);
    assert.match(engineReleaseBuild, /finally \{\s*cleanBuildIntermediates\(shellRoot\);/s);
    const runtimeMapping = packageJson.build.win.extraResources.find(
      (entry) => entry.to === "engine-manifest.json",
    );
    const engineArchiveMapping = packageJson.build.win.extraResources.find(
      (entry) => entry.to === "Kosmos Engine.zip",
    );
    assert.deepEqual(runtimeMapping, {
      from: ".tmp/engine.next/engine-manifest.json",
      to: "engine-manifest.json",
    });
    assert.deepEqual(engineArchiveMapping, {
      from: ".tmp/engine.next/Kosmos-Engine.zip",
      to: "Kosmos Engine.zip",
    });
    assert.equal(
      readFileSync(path.join(stageDir, "kepler-backend.exe"), "utf8"),
      "fresh:kepler-backend",
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("build retention clears only disposable next outputs and protects active builds", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "kosmos-runtime-stage-"));
  try {
    mkdirSync(path.join(root, ".tmp", "runtime.next"), { recursive: true });
    mkdirSync(path.join(root, ".tmp", "engine.next"), { recursive: true });
    writeFileSync(path.join(root, ".tmp", "runtime.next", "stale"), "stale");
    cleanBuildIntermediates(root);
    assert.equal(existsSync(path.join(root, ".tmp", "runtime.next")), false);
    const release = acquireBuildLock(root);
    assert.throws(() => acquireBuildLock(root), /build already active/);
    release();
    assert.equal(existsSync(path.join(root, ".tmp", "build.active.lock")), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
