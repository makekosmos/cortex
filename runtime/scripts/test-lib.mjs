import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const cortex = resolve(import.meta.dirname, "..", "..");
const target = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : resolve(cortex, "target");
const executable = (name) =>
  resolve(target, "debug", `${name}${process.platform === "win32" ? ".exe" : ""}`);
const bridge = executable("ark-markdown-bridge");
// Cargo's default job count is used: a cold `--workspace` run took 1474 s with
// CARGO_BUILD_JOBS=1 and 498 s with 12, and free memory stayed above 17.5 GB
// of 32 (docs/experiments/2026-09-30-build-speed.md). Set CARGO_BUILD_JOBS to
// limit it on a smaller machine.
function run(cwd, args) {
  const result = spawnSync("cargo", args, { cwd, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

// These are integration-test fixtures, not production dependencies. Build
// them explicitly so `cargo test -p engine --lib` has the same
// prerequisites in the standalone Makekosmos layout as in CI.
run(cortex, [
  "build",
  "-p",
  "engine",
  "--bin",
  "ark-markdown-bridge",
  "--features",
  "markdown-bridge-fixture",
]);

if (!existsSync(bridge)) {
  throw new Error(`ark-markdown-bridge fixture was not produced: ${bridge}`);
}

const workspace = process.argv[2] === "--workspace";
if (workspace) {
  run(cortex, ["test", "--workspace", "--lib"]);
  // Integration tests: one `it` binary per crate (tests/it/main.rs).
  // engine/iroh-spike: the integration replication tests exercise the
  // in-process ARK service's iroh transport (previously provided by the
  // separately-built ark-core-rpc fixture binary). ark-core runs on its own
  // below, with its default features, as upstream does.
  // engine/markdown-bridge-fixture: builds the bridge fixture bin so the
  // integration tests get CARGO_BIN_EXE_ark-markdown-bridge.
  run(cortex, [
    "test",
    "--workspace",
    "--exclude",
    "ark-core",
    "--test",
    "*",
    "--features",
    "engine/iroh-spike",
    "--features",
    "engine/markdown-bridge-fixture",
  ]);
  run(cortex, ["test", "-p", "ark-core"]);
  run(cortex, ["test", "-p", "engine", "--bins", "--features", "markdown-bridge-fixture"]);
} else {
  run(cortex, ["test", "-p", "engine", "--lib", ...process.argv.slice(2)]);
}
