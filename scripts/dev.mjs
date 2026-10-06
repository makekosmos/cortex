#!/usr/bin/env node
// One-command local dev: builds the Engine, then runs the Manager against it.
//
//   pnpm run dev                    # or: node scripts/dev.mjs
//   node scripts/dev.mjs --engine-only
//   node scripts/dev.mjs --data-dir /tmp/other
//
// The Manager checks Engine health at startup and spawns the binary named by
// MUNDUS_ENGINE_PATH when none is running (manager-gpui/src/boot.rs), so the
// only things this wrapper owns are the shared MUNDUS_DATA_DIR and the path.
// The Engine is deliberately detached from the Manager: closing the Manager
// leaves it running; stop it with `kill $(jq .pid <data-dir>/engine.lock.json)`.
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const dataDirAt = args.indexOf("--data-dir");
const dataDir =
  (dataDirAt >= 0 ? args[dataDirAt + 1] : undefined) ??
  process.env.MUNDUS_DATA_DIR ??
  join(tmpdir(), "mundus-dev");

function cargo(cargoArgs, env = {}) {
  const result = spawnSync("cargo", cargoArgs, {
    stdio: "inherit",
    env: { ...process.env, ...env },
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

const metadata = spawnSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
  encoding: "utf8",
});
if (metadata.status !== 0) {
  console.error(metadata.stderr);
  process.exit(metadata.status ?? 1);
}
const { target_directory: targetDir } = JSON.parse(metadata.stdout);
const exe = process.platform === "win32" ? "mundus-engine.exe" : "mundus-engine";
const engine = join(targetDir, "debug", exe);
const env = { MUNDUS_DATA_DIR: dataDir, MUNDUS_ENGINE_PATH: engine };

console.error(`data dir: ${dataDir}`);
if (flag("--engine-only")) {
  cargo(["run", "-p", "engine"], env);
} else {
  cargo(["build", "-p", "engine", "--bin", "mundus-engine"]);
  cargo(["run", "--manifest-path", "manager-gpui/Cargo.toml"], env);
}
