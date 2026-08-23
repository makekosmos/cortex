import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const cortex = resolve(import.meta.dirname, "..", "..");
const core = resolve(cortex, "..", "core");
const target = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : resolve(cortex, "target");
const coreTarget = process.env.CARGO_TARGET_DIR
  ? target
  : resolve(core, "target");
const executable = (name) =>
  resolve(target, "debug", `${name}${process.platform === "win32" ? ".exe" : ""}`);
const coreRpc = resolve(
  coreTarget,
  "debug",
  `ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`,
);
const bridge = executable("ark-markdown-bridge");
const env = {
  ...process.env,
  // The standalone Core build is memory-heavy on Windows CI/dev machines.
  // Keep the preflight deterministic unless the caller explicitly opts in.
  CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? "1",
};

function run(cwd, args) {
  const result = spawnSync("cargo", args, { cwd, stdio: "inherit", env });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

// These are integration-test fixtures, not production dependencies. Build
// them explicitly so `cargo test -p kepler-backend --lib` has the same
// prerequisites in the standalone Makekosmos layout as in CI.
run(core, ["build", "-p", "ark-core", "--bin", "ark-core-rpc"]);
run(cortex, ["build", "-p", "kepler-backend", "--bin", "ark-markdown-bridge"]);

const coreRpcPath = process.env.ARK_CORE_RPC_PATH ?? coreRpc;
if (!existsSync(coreRpcPath)) {
  throw new Error(`ark-core-rpc fixture was not produced: ${coreRpcPath}`);
}
if (!existsSync(bridge)) {
  throw new Error(`ark-markdown-bridge fixture was not produced: ${bridge}`);
}

run(cortex, ["test", "-p", "kepler-backend", "--lib", ...process.argv.slice(2)]);
