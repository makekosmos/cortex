#!/usr/bin/env node
import { execFileSync, spawnSync } from "node:child_process";
import path from "node:path";
import { copyFileSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  effectiveCargoTargetDir,
  RUNTIME_BINARIES,
  stageRuntimeBinary,
  acquireBuildLock,
  cleanBuildIntermediates,
} from "./runtime-staging.mjs";
import { buildEnginePayload } from "./engine-distribution.mjs";

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

// KOS-233: one product, one version. The Engine no longer has its own
// per-Engine version config file — it reports the Mundus Desktop product
// version (`desktop/release-versions.json`) and the commit it was built from.
// Both are baked into the binary at compile time via `option_env!`
// (see runtime/src/build_info.rs), so they must be set before the `cargo build`
// calls below, on every platform.
const releaseVersions = JSON.parse(
  readFileSync(path.join(shellRoot, "release-versions.json"), "utf8"),
);
const productVersion = process.platform === "win32" ? releaseVersions.win : releaseVersions.mac;
const sourceCommit = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: shellRoot,
  encoding: "utf8",
}).trim();
const cargoEnv = {
  ...process.env,
  MUNDUS_PRODUCT_VERSION: productVersion,
  MUNDUS_ENGINE_SOURCE_COMMIT: sourceCommit,
};

const cortexBuildArgs = ["build", "--release", "--manifest-path", "../Cargo.toml"];
const buildMundus = spawnSync(
  "cargo",
  [...cortexBuildArgs, "--bin", "mundus-engine", "--features", "windows-gui-subsystem"],
  {
    cwd: shellRoot,
    stdio: "inherit",
    windowsHide: true,
    env: cargoEnv,
  },
);
if ((buildMundus.status ?? 1) !== 0) process.exit(buildMundus.status ?? 1);

// Package only binaries produced by this build. In particular, release builds use an
// alternate CARGO_TARGET_DIR to avoid locks from installed services; package.json used to
// ignore it and silently ship stale binaries from repoRoot/target/release.
const stageDir = path.join(shellRoot, ".tmp", "runtime.next");
try {
  for (const bin of RUNTIME_BINARIES) {
    stageRuntimeBinary(bin, path.join(cortexTargetDir, "release"), stageDir);
  }
  copyFileSync(path.join(shellRoot, "build", "tray.ico"), path.join(stageDir, "tray.ico"));
} catch (error) {
  console.error(`[build-backend] ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
// KOS-233: the Windows Desktop installer ships the Engine it was built with —
// packaged from this same `stageDir`/commit, never downloaded from a
// published release. KOS-306: the payload is staged unpacked (a directory of
// the manifest-listed files plus engine-manifest.json) — the installer runs
// the staged exe's `install` subcommand directly, no zip and no PowerShell.
let engineVersion = null;
if (process.platform === "win32") {
  engineVersion = productVersion;
  const engineDir = path.join(shellRoot, ".tmp", "engine.next");
  buildEnginePayload(stageDir, engineDir, {
    version: engineVersion,
    sourceCommit,
  });
}
console.log(`[build-backend] staged Cortex and ARK runtime binaries`);
if (engineVersion) console.log(`[build-backend] staged Engine ${engineVersion} (built from tree)`);
