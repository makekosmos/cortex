#!/usr/bin/env node
// Dev-flow backend build for `bun run dev` (debug profile, fast iterate).
//
// Default behavior is byte-for-byte the same as the previous inline
// package.json command: `cargo build --manifest-path ../Cargo.toml --bin
// kepler-backend`.
//
// iroh-spike dev step: when `KOSMOS_IROH` is set (same env var
// `platform/runtime/src/sync.rs::start_lan_sync()` reads to decide
// `use_iroh`), also (re)build `ark-core-rpc` with `--features iroh-spike` so
// `ArkHost::resolve_ark_core_rpc_path()`'s dev fallback
// (`target/debug/ark-core-rpc[.exe]`) finds a sidecar binary that
// actually understands `use_iroh`/`iroh_peer_ticket` in StartSync — without
// this, KOSMOS_IROH=1 against a non-iroh-spike sidecar gets a clean rejection
// error (see main.rs handle_start_sync), not a working iroh transport.
//
// The sidecar is always provisioned because a fresh Cortex checkout has no
// sibling Core target directory. The iroh feature is opt-in to keep the
// default development build smaller.
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { effectiveCargoTargetDir } from "./runtime-staging.mjs";
import { ensureArkCoreRpc } from "./ark-core-rpc.mjs";

const shellRoot = fileURLToPath(new URL("..", import.meta.url));
const cortexRoot = path.resolve(shellRoot, "..");
const targetDir = effectiveCargoTargetDir(shellRoot, cortexRoot, process.env.CARGO_TARGET_DIR);

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

run(["build", "--manifest-path", "../Cargo.toml", "--bin", "kepler-backend"]);

const irohRequested =
  process.env.KOSMOS_IROH === "1" || /^true$/i.test(process.env.KOSMOS_IROH ?? "");
ensureArkCoreRpc({
  debug: true,
  features: irohRequested ? ["iroh-spike"] : [],
  prebuiltManifest: process.env.ARK_CORE_RPC_PREBUILT,
  targetDir: path.join(targetDir, "debug"),
});

if (irohRequested) {
  console.log(
    "[build:backend:dev] KOSMOS_IROH set — using ark-core-rpc with --features iroh-spike",
  );
}
