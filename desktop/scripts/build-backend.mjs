#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  effectiveCargoTargetDir,
  RUNTIME_BINARIES,
  stageRuntimeBinaries,
} from "./runtime-staging.mjs";

const shellRoot = fileURLToPath(new URL("..", import.meta.url));
const repoRoot = path.resolve(shellRoot, "../..");

const baseArgs = ["build", "--release", "--manifest-path", "../../Cargo.toml"];
const buildKepler = spawnSync(
  "cargo",
  [...baseArgs, "--bin", "kepler-backend", "--features", "windows-gui-subsystem"],
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
    ...baseArgs,
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
  const result = spawnSync("cargo", [...baseArgs, "--bin", bin], {
    cwd: shellRoot,
    stdio: "inherit",
    windowsHide: true,
  });
  if ((result.status ?? 1) !== 0) process.exit(result.status ?? 1);
}

// Package only binaries produced by this build. In particular, release builds use an
// alternate CARGO_TARGET_DIR to avoid locks from installed services; package.json used to
// ignore it and silently ship stale binaries from repoRoot/target/release.
const cargoTargetDir = effectiveCargoTargetDir(shellRoot, repoRoot, process.env.CARGO_TARGET_DIR);
const releaseDir = path.join(cargoTargetDir, "release");
const stageDir = path.join(shellRoot, ".tmp", "runtime");
try {
  stageRuntimeBinaries(releaseDir, stageDir);
} catch (error) {
  console.error(`[build-backend] ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
console.log(`[build-backend] staged runtime binaries from ${releaseDir}`);
