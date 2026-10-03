#!/usr/bin/env node
// Host-native Engine + Manager builds share the same release metadata on every OS.
// Packaging/installing is separate; this does not enable login autostart.
import { execFileSync, spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readReleaseVersion } from "./release-version.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

export function nativeBuildPlan({
  root = ROOT,
  dev = false,
  sourceCommit = "",
  inheritedEnv = process.env,
} = {}) {
  const env = {
    ...inheritedEnv,
    MUNDUS_PRODUCT_VERSION: dev ? "" : readReleaseVersion({ root }),
    MUNDUS_BUILD_CHANNEL: dev ? "dev" : "stable",
    MUNDUS_ENGINE_SOURCE_COMMIT: sourceCommit,
  };
  return [
    {
      cwd: root,
      args: ["build", "--locked", "--release", "-p", "engine", "--bin", "mundus-engine"],
      env,
    },
    { cwd: path.join(root, "manager-gpui"), args: ["build", "--locked", "--release"], env },
  ];
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const dev = process.argv.includes("--dev");
  const sourceCommit = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: ROOT,
    encoding: "utf8",
  }).trim();
  const plan = nativeBuildPlan({ dev, sourceCommit });
  console.log(
    `[native-build] ${plan[0].env.MUNDUS_PRODUCT_VERSION || "crate version"} (${plan[0].env.MUNDUS_BUILD_CHANNEL})`,
  );
  for (const step of plan) {
    const result = spawnSync("cargo", step.args, {
      cwd: step.cwd,
      env: step.env,
      stdio: "inherit",
    });
    if (result.error) {
      console.error(result.error.message);
      process.exit(1);
    }
    if (result.status !== 0) process.exit(result.status ?? 1);
  }
}
