#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  effectiveCargoTargetDir,
  RUNTIME_BINARIES,
  stageRuntimeBinary,
} from "./runtime-staging.mjs";
import { ensureArkCoreRpc } from "./ark-core-rpc.mjs";

const shellRoot = fileURLToPath(new URL("..", import.meta.url));
const cortexRoot = path.resolve(shellRoot, "..");
const cortexTargetDir = effectiveCargoTargetDir(shellRoot, cortexRoot, process.env.CARGO_TARGET_DIR);

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
const stageDir = path.join(shellRoot, ".tmp", "runtime");
try {
  for (const bin of [RUNTIME_BINARIES[0], ...RUNTIME_BINARIES.slice(2)]) {
    stageRuntimeBinary(bin, path.join(cortexTargetDir, "release"), stageDir);
  }
  stageRuntimeBinary(RUNTIME_BINARIES[1], path.join(cortexTargetDir, "release"), stageDir);
} catch (error) {
  console.error(`[build-backend] ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
console.log(`[build-backend] staged Cortex and ARK runtime binaries`);
