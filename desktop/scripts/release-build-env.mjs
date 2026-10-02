// One compile-time identity for every packaged cargo build: the Windows
// installer scripts and the mac bundle. A bare `cargo build` does not go
// through here, so it does not claim a product version (the binaries then
// report "dev" — see runtime/src/build_info.rs).
import { execFileSync } from "node:child_process";
import { readReleaseVersion } from "./release-version.mjs";

const SOURCE_COMMIT = /^[0-9a-f]{40}$/;

export function releaseBuildIdentity(cwd, baseEnv = process.env) {
  const productVersion = readReleaseVersion();
  const sourceCommit = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd,
    encoding: "utf8",
  }).trim();
  if (!SOURCE_COMMIT.test(sourceCommit))
    throw new Error(`engine source commit must be 40 hex chars, got ${JSON.stringify(sourceCommit)}`);
  return {
    productVersion,
    sourceCommit,
    env: {
      ...baseEnv,
      MUNDUS_PRODUCT_VERSION: productVersion,
      MUNDUS_ENGINE_SOURCE_COMMIT: sourceCommit,
    },
  };
}
