#!/usr/bin/env node
// Dev-flow backend build for `pnpm run dev` (debug profile, fast iterate).
//
// Default behavior is byte-for-byte the same as the previous inline
// package.json command: `cargo build --manifest-path ../Cargo.toml --bin
// mundus-engine`.
//
// The Engine hosts ark-core in-process — there is no sidecar to provision.
// iroh-spike dev step: when `MUNDUS_IROH` is set (same env var
// `platform/runtime/src/sync.rs::start_lan_sync()` reads to decide
// `use_iroh`), mundus-engine itself is built with `--features iroh-spike`
// so the embedded ARK service understands `use_iroh`/`iroh_peer_ticket` in
// StartSync. The feature stays opt-in to keep the default development build
// smaller.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { env } from "./brand.mjs";

const shellRoot = fileURLToPath(new URL("..", import.meta.url));

function run(args) {
  const result = spawnSync("cargo", args, {
    cwd: shellRoot,
    stdio: "inherit",
    windowsHide: true,
  });
  if ((result.status ?? 1) !== 0) {
    process.exit(result.status ?? 1);
  }
}

const irohRequested = env("IROH") === "1" || /^true$/i.test(env("IROH") ?? "");
const engineArgs = ["build", "--manifest-path", "../Cargo.toml", "--bin", "mundus-engine"];
if (irohRequested) engineArgs.push("--features", "iroh-spike");
run(engineArgs);

if (irohRequested) {
  console.log(
    "[build:backend:dev] MUNDUS_IROH set — built mundus-engine with --features iroh-spike",
  );
}
