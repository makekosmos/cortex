import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { ensureArkCoreRpc } from "../../desktop/scripts/ark-core-rpc.mjs";

const cortex = resolve(import.meta.dirname, "..", "..");
const target = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : resolve(cortex, "target");
const executable = (name) =>
  resolve(target, "debug", `${name}${process.platform === "win32" ? ".exe" : ""}`);
const coreRpc =
  process.env.ARK_CORE_RPC_PATH ??
  ensureArkCoreRpc({
    debug: true,
    features: ["iroh-spike"],
    targetDir: resolve(target, "debug"),
  });
const bridge = executable("ark-markdown-bridge");
const env = {
  ...process.env,
  ARK_CORE_RPC_PATH: coreRpc,
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
run(cortex, ["build", "-p", "kepler-backend", "--bin", "ark-markdown-bridge"]);

const coreRpcPath = coreRpc;
if (!existsSync(coreRpcPath)) {
  throw new Error(`ark-core-rpc fixture was not produced: ${coreRpcPath}`);
}
if (!existsSync(bridge)) {
  throw new Error(`ark-markdown-bridge fixture was not produced: ${bridge}`);
}

const workspace = process.argv[2] === "--workspace";
if (workspace) {
  run(cortex, ["test", "--workspace", "--lib"]);
  // ark-core's iroh_bidirectional_network target requires the iroh-spike
  // feature; an explicit --test wildcard makes cargo error on it instead of
  // skipping. Run ark-core with default target selection (same coverage as
  // upstream's `cargo test --manifest-path crates/ark-core/Cargo.toml`).
  run(cortex, ["test", "--workspace", "--exclude", "ark-core", "--test", "*"]);
  run(cortex, ["test", "-p", "ark-core"]);
  run(cortex, ["test", "-p", "kepler-backend", "--bins"]);
} else {
  run(cortex, ["test", "-p", "kepler-backend", "--lib", ...process.argv.slice(2)]);
}
