#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  effectiveCargoTargetDir,
  RUNTIME_BINARIES,
  stageRuntimeBinary,
  acquireBuildLock,
  cleanBuildIntermediates,
} from "./runtime-staging.mjs";
import { ensureArkCoreRpc } from "./ark-core-rpc.mjs";
import { buildEngineArchive, verifyEngineArchive } from "./engine-distribution.mjs";
import { getVersion } from "./release-version.mjs";

const shellRoot = fileURLToPath(new URL("..", import.meta.url));
const releaseBuildLock = acquireBuildLock(shellRoot);
cleanBuildIntermediates(shellRoot);
process.on("exit", releaseBuildLock);
process.on("exit", (code) => {
  if (code !== 0) cleanBuildIntermediates(shellRoot);
});
const cortexRoot = path.resolve(shellRoot, "..");
const cortexTargetDir = effectiveCargoTargetDir(
  shellRoot,
  cortexRoot,
  process.env.CARGO_TARGET_DIR,
);

const cortexBuildArgs = ["build", "--release", "--manifest-path", "../Cargo.toml"];
const buildKepler = spawnSync(
  "cargo",
  [...cortexBuildArgs, "--bin", "kepler-backend", "--features", "windows-gui-subsystem"],
  {
    cwd: shellRoot,
    stdio: "inherit",
    windowsHide: true,
  },
);
if ((buildKepler.status ?? 1) !== 0) process.exit(buildKepler.status ?? 1);
ensureArkCoreRpc({
  features: ["iroh-spike", "windows-gui-subsystem"],
  targetDir: path.join(cortexTargetDir, "release"),
});
for (const bin of RUNTIME_BINARIES.slice(2)) {
  const result = spawnSync("cargo", [...cortexBuildArgs, "--bin", bin], {
    cwd: shellRoot,
    stdio: "inherit",
    windowsHide: true,
  });
  if ((result.status ?? 1) !== 0) process.exit(result.status ?? 1);
}

// Package only binaries produced by this build. In particular, release builds use an
// alternate CARGO_TARGET_DIR to avoid locks from installed services; package.json used to
// ignore it and silently ship stale binaries from repoRoot/target/release.
const stageDir = path.join(shellRoot, ".tmp", "runtime.next");
try {
  for (const bin of [RUNTIME_BINARIES[0], ...RUNTIME_BINARIES.slice(2)]) {
    stageRuntimeBinary(bin, path.join(cortexTargetDir, "release"), stageDir);
  }
  stageRuntimeBinary(RUNTIME_BINARIES[1], path.join(cortexTargetDir, "release"), stageDir);
} catch (error) {
  console.error(`[build-backend] ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
let engineVersion = null;
if (process.platform === "win32") {
  engineVersion = process.env.KOSMOS_ENGINE_VERSION ?? getVersion("win");
  const engineDir = path.join(shellRoot, ".tmp", "engine.next");
  const engineArchive = path.join(engineDir, "Kosmos-Engine.zip");
  mkdirSync(engineDir, { recursive: true });
  const engineUrl =
    process.env.KOSMOS_ENGINE_RELEASE_URL ??
    `https://github.com/makekosmos/desktop/releases/download/v${engineVersion}/Kosmos-Engine-${engineVersion}.zip`;
  let engineManifest;
  if (process.env.KOSMOS_ENGINE_REUSE_ARCHIVE) {
    const sourceArchive = process.env.KOSMOS_ENGINE_REUSE_ARCHIVE;
    const sourceManifest = process.env.KOSMOS_ENGINE_REUSE_MANIFEST;
    if (!sourceManifest) throw new Error("KOSMOS_ENGINE_REUSE_MANIFEST is required");
    copyFileSync(sourceArchive, engineArchive);
    engineManifest = JSON.parse(readFileSync(sourceManifest, "utf8"));
    if (
      engineManifest.version !== engineVersion ||
      !verifyEngineArchive(engineArchive, engineManifest)
    )
      throw new Error(`reused Engine does not match ${engineVersion}`);
  } else {
    engineManifest = buildEngineArchive(stageDir, engineArchive, {
      version: engineVersion,
      url: engineUrl,
    });
  }
  engineManifest.channel_url =
    "https://github.com/makekosmos/desktop/releases/latest/download/Kosmos-Engine-manifest.json";
  writeFileSync(
    path.join(engineDir, "engine-manifest.json"),
    JSON.stringify(engineManifest, null, 2) + "\n",
  );
}
console.log(`[build-backend] staged Cortex and ARK runtime binaries`);
if (engineVersion) console.log(`[build-backend] staged standalone engine ${engineVersion}`);
