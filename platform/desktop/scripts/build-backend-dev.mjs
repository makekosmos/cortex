#!/usr/bin/env node
// Dev-flow backend build for `bun run dev` (debug profile, fast iterate).
//
// Default behavior is byte-for-byte the same as the previous inline
// package.json command: `cargo build --manifest-path ../../Cargo.toml --bin
// kepler-backend`.
//
// iroh-spike dev step: when `KOSMOS_IROH` is set (same env var
// `platform/runtime/src/sync.rs::start_lan_sync()` reads to decide
// `use_iroh`), also (re)build `ark-core-rpc` with `--features iroh-spike` so
// `ArkHost::resolve_ark_core_rpc_path()`'s dev fallback
// (`../../target/debug/ark-core-rpc[.exe]`) finds a sidecar binary that
// actually understands `use_iroh`/`iroh_peer_ticket` in StartSync — without
// this, KOSMOS_IROH=1 against a non-iroh-spike sidecar gets a clean rejection
// error (see main.rs handle_start_sync), not a working iroh transport.
//
// Without KOSMOS_IROH set, ark-core-rpc is NOT built here (matches prior
// behavior) — a dev run that never touches sync still doesn't pay for it,
// and an existing manually-built sidecar from a previous `build:backend` run
// stays untouched.
import { spawnSync } from "node:child_process";

function run(args) {
  const result = spawnSync("cargo", args, {
    cwd: new URL("..", import.meta.url),
    stdio: "inherit",
  });
  if ((result.status ?? 1) !== 0) {
    process.exit(result.status ?? 1);
  }
}

run(["build", "--manifest-path", "../../Cargo.toml", "--bin", "kepler-backend"]);

const irohRequested =
  process.env.KOSMOS_IROH === "1" || /^true$/i.test(process.env.KOSMOS_IROH ?? "");
if (irohRequested) {
  console.log(
    "[build:backend:dev] KOSMOS_IROH set — building ark-core-rpc with --features iroh-spike",
  );
  run([
    "build",
    "--manifest-path",
    "../../Cargo.toml",
    "-p",
    "ark-core",
    "--bin",
    "ark-core-rpc",
    "--features",
    "iroh-spike",
  ]);
}
