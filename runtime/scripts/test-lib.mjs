import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { openGateTmp, sweepGateTmp } from "./gate-tmp.mjs";

const cortex = resolve(import.meta.dirname, "..", "..");
const target = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : resolve(cortex, "target");
const executable = (name) =>
  resolve(target, "debug", `${name}${process.platform === "win32" ? ".exe" : ""}`);
const bridge = executable("ark-markdown-bridge");
// Tests must not write to the user's %TEMP% (KOS-270): every cargo
// invocation gets a fresh per-run scratch dir as TMP/TEMP/TMPDIR, swept and
// reported once the test processes have exited and released their handles.
const gateTmp = openGateTmp(target, process.pid);
process.on("exit", () => {
  const removed = sweepGateTmp(target, gateTmp);
  console.log(
    `gate tmp: swept ${removed} leftover entr${removed === 1 ? "y" : "ies"} under ${gateTmp}`,
  );
});
const gateEnv = { ...process.env, TMP: gateTmp, TEMP: gateTmp, TMPDIR: gateTmp };
// Cargo's default job count is used: a cold `--workspace` run took 1474 s with
// CARGO_BUILD_JOBS=1 and 498 s with 12, and free memory stayed above 17.5 GB
// of 32 (docs/experiments/2026-09-30-build-speed.md). Set CARGO_BUILD_JOBS to
// limit it on a smaller machine.
function run(cwd, args) {
  const result = spawnSync("cargo", args, { cwd, stdio: "inherit", env: gateEnv });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

// One feature set for the whole gate (KOS-270): the gate must test exactly
// what ships, so every crate is compiled once with the same flags.
// The engine features below enable the two test-only fixture binaries and
// the loopback-origin broker path the engine integration tests exercise;
// the compile_error in runtime/src/lib.rs keeps them out of release builds.
// ark-core needs no feature flags: iroh is a regular dependency now.
const GATE_FEATURES = [
  "--features",
  "engine/package-worker-fixture,engine/markdown-bridge-fixture",
];

// These are integration-test fixtures, not production dependencies. Build
// them explicitly with the same feature set so `cargo test` reuses the
// artifacts instead of recompiling, and so `cargo test -p engine` has the
// same prerequisites in the standalone Makekosmos layout as in CI.
run(cortex, ["build", "-p", "engine", "--bins", ...GATE_FEATURES]);

if (!existsSync(bridge)) {
  throw new Error(`ark-markdown-bridge fixture was not produced: ${bridge}`);
}

const workspace = process.argv[2] === "--workspace";
if (workspace) {
  // A single cargo invocation: lib tests, bin tests and the per-crate `it`
  // integration binaries all share one compilation of each crate.
  run(cortex, ["test", "--workspace", ...GATE_FEATURES]);
} else {
  run(cortex, ["test", "-p", "engine", "--lib", ...GATE_FEATURES, ...process.argv.slice(2)]);
}
