#!/usr/bin/env node
// Dev-flow backend build for `pnpm run dev` (debug profile, fast iterate).
//
// `cargo build --manifest-path ../Cargo.toml --bin mundus-engine`.
// The Engine hosts ark-core in-process — there is no sidecar to provision —
// and iroh is a regular ark-core dependency, so the debug build matches the
// released feature set with no extra flags.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const shellRoot = fileURLToPath(new URL("..", import.meta.url));

const result = spawnSync(
  "cargo",
  ["build", "--manifest-path", "../Cargo.toml", "--bin", "mundus-engine"],
  {
    cwd: shellRoot,
    stdio: "inherit",
    windowsHide: true,
  },
);
if ((result.status ?? 1) !== 0) {
  process.exit(result.status ?? 1);
}
