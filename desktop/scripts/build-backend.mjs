#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  effectiveCargoTargetDir,
  RUNTIME_BINARIES,
  stageRuntimeBinary,
} from "./runtime-staging.mjs";

const shellRoot = fileURLToPath(new URL("..", import.meta.url));
const cortexRoot = path.resolve(shellRoot, "..");
const coreRoot = path.resolve(cortexRoot, "..", "core");

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
const buildArk = spawnSync(
  "cargo",
  [
    "build",
    "--release",
    "--manifest-path",
    "../../core/Cargo.toml",
    "-p",
    "ark-core",
    "--bin",
    "ark-core-rpc",
    "--features",
    "iroh-spike,windows-gui-subsystem",
  ],
  { cwd: shellRoot, stdio: "inherit", windowsHide: true },
);
if ((buildArk.status ?? 1) !== 0) process.exit(buildArk.status ?? 1);
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
const cortexTargetDir = effectiveCargoTargetDir(shellRoot, cortexRoot, process.env.CARGO_TARGET_DIR);
const coreTargetDir = effectiveCargoTargetDir(shellRoot, coreRoot, process.env.CARGO_TARGET_DIR);
const stageDir = path.join(shellRoot, ".tmp", "runtime");
try {
  for (const bin of [RUNTIME_BINARIES[0], ...RUNTIME_BINARIES.slice(2)]) {
    stageRuntimeBinary(bin, path.join(cortexTargetDir, "release"), stageDir);
  }
  stageRuntimeBinary(RUNTIME_BINARIES[1], path.join(coreTargetDir, "release"), stageDir);
} catch (error) {
  console.error(`[build-backend] ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
console.log(`[build-backend] staged Cortex and Core runtime binaries`);
