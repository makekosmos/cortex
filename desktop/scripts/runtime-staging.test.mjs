import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  effectiveCargoTargetDir,
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
  assert.match(ARK_CORE_REVISION, /^[0-9a-f]{40}$/);

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
    const runtimeMapping = packageJson.build.win.extraResources.find(
      (entry) => entry.to === "Kosmos Runtime.exe",
    );
    assert.deepEqual(runtimeMapping, {
      from: ".tmp/runtime/kepler-backend.exe",
      to: "Kosmos Runtime.exe",
    });
    assert.equal(
      readFileSync(path.join(stageDir, "kepler-backend.exe"), "utf8"),
      "fresh:kepler-backend",
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
