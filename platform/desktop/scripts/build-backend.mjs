#!/usr/bin/env node
import { spawnSync } from "node:child_process";

const baseArgs = ["build", "--release", "--manifest-path", "../../Cargo.toml"];
const buildKepler = spawnSync("cargo", [...baseArgs, "--bin", "kepler-backend"], {
  cwd: new URL("..", import.meta.url),
  stdio: "inherit",
});
if ((buildKepler.status ?? 1) !== 0) process.exit(buildKepler.status ?? 1);
const buildLocalStt = spawnSync("cargo", [...baseArgs, "--bin", "kosmos-local-stt"], {
  cwd: new URL("..", import.meta.url),
  stdio: "inherit",
});
if ((buildLocalStt.status ?? 1) !== 0) process.exit(buildLocalStt.status ?? 1);
const buildArk = spawnSync(
  "cargo",
  [...baseArgs, "-p", "ark-core", "--bin", "ark-core-rpc", "--features", "iroh-spike"],
  { cwd: new URL("..", import.meta.url), stdio: "inherit" },
);
if ((buildArk.status ?? 1) !== 0) process.exit(buildArk.status ?? 1);
for (const bin of ["kepler-focus-helper", "kepler-focus-svc"]) {
  const result = spawnSync("cargo", [...baseArgs, "--bin", bin], {
    cwd: new URL("..", import.meta.url),
    stdio: "inherit",
  });
  if ((result.status ?? 1) !== 0) process.exit(result.status ?? 1);
}
