#!/usr/bin/env node
// Dev-flow backend build for `pnpm run dev` (debug profile, fast iterate).
//
// Default behavior is byte-for-byte the same as the previous inline
// package.json command: `cargo build --manifest-path ../Cargo.toml --bin
// kepler-backend`.
//
// The Engine hosts ark-core in-process — there is no sidecar to provision.
// iroh-spike dev step: when `KOSMOS_IROH` is set (same env var
// `platform/runtime/src/sync.rs::start_lan_sync()` reads to decide
// `use_iroh`), kepler-backend itself is built with `--features iroh-spike`
// so the embedded ARK service understands `use_iroh`/`iroh_peer_ticket` in
// StartSync. The feature stays opt-in to keep the default development build
// smaller.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

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

const irohRequested =
  process.env.KOSMOS_IROH === "1" || /^true$/i.test(process.env.KOSMOS_IROH ?? "");
const keplerArgs = ["build", "--manifest-path", "../Cargo.toml", "--bin", "kepler-backend"];
if (irohRequested) keplerArgs.push("--features", "iroh-spike");
run(keplerArgs);

if (irohRequested) {
  console.log(
    "[build:backend:dev] KOSMOS_IROH set — built kepler-backend with --features iroh-spike",
  );
}
