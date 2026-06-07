#!/usr/bin/env node
import { spawnSync } from "node:child_process";

const bins = ["kepler-backend", "ark-core-rpc", "kepler-focus-helper", "kepler-focus-svc"];
const args = ["build", "--release", "--manifest-path", "../../Cargo.toml"];
for (const bin of bins) {
  args.push("--bin", bin);
}

const result = spawnSync("cargo", args, {
  cwd: new URL("..", import.meta.url),
  stdio: "inherit",
});

process.exit(result.status ?? 1);
