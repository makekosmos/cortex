import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const ARK_CORE_REPOSITORY = "https://github.com/makekosmos/core.git";
// Keep the Rust API and sidecar binary on the same immutable Core revision.
export const ARK_CORE_REVISION = "717eabbfb3cd302569b52890b717c8a4da1d8709";

const shellRoot = fileURLToPath(new URL("..", import.meta.url));
const cacheRoot = path.join(shellRoot, ".tmp", "ark-core-rpc");

function installRoot(debug, features) {
  const profile = debug ? "debug" : "release";
  const featureKey = features.length === 0 ? "default" : [...features].sort().join("+");
  return path.join(cacheRoot, ARK_CORE_REVISION, profile, featureKey);
}

export function ensureArkCoreRpc({ debug = false, features = [], targetDir } = {}) {
  const root = installRoot(debug, features);
  const suffix = process.platform === "win32" ? ".exe" : "";
  const installed = path.join(root, "bin", `ark-core-rpc${suffix}`);
  if (!existsSync(installed)) {
    mkdirSync(root, { recursive: true });
    const args = [
      "install",
      "--git",
      ARK_CORE_REPOSITORY,
      "--rev",
      ARK_CORE_REVISION,
      "ark-core",
      "--bin",
      "ark-core-rpc",
      "--root",
      root,
      "--locked",
    ];
    if (debug) args.push("--debug");
    if (features.length > 0) args.push("--features", features.join(","));
    const result = spawnSync("cargo", args, {
      cwd: shellRoot,
      env: { ...process.env, CARGO_TARGET_DIR: path.join(root, "target") },
      stdio: "inherit",
      windowsHide: true,
    });
    if ((result.status ?? 1) !== 0) process.exit(result.status ?? 1);
  }
  if (!existsSync(installed)) {
    throw new Error(`cargo install did not produce ${installed}`);
  }
  if (targetDir) {
    mkdirSync(targetDir, { recursive: true });
    const target = path.join(targetDir, `ark-core-rpc${suffix}`);
    copyFileSync(installed, target);
    return target;
  }
  return installed;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))
) {
  const debug = process.argv.includes("--debug");
  const targetIndex = process.argv.indexOf("--target-dir");
  const targetDir = targetIndex === -1 ? undefined : process.argv[targetIndex + 1];
  console.log(ensureArkCoreRpc({ debug, targetDir }));
}
