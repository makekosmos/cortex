import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { openGateTmp, reportGateTmpSweep } from "./gate-tmp.mjs";

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
process.on("exit", (code) => {
  if (reportGateTmpSweep(target, gateTmp) && code === 0) {
    console.error("gate tmp: leftover entries are a leak — the gate fails");
    process.exitCode = 1;
  }
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
// them explicitly with the same feature set so the test run reuses the
// artifacts instead of recompiling, and so `-p engine` alone has the
// same prerequisites in the standalone Makekosmos layout as in CI.
run(cortex, ["build", "-p", "engine", "--bins", ...GATE_FEATURES]);

if (!existsSync(bridge)) {
  throw new Error(`ark-markdown-bridge fixture was not produced: ${bridge}`);
}

// Tests run through cargo-nextest, one process per test, in parallel across
// every test binary: `cargo test` runs the binaries one after another, so the
// gate waited on the slowest binary's tail (157 s against 77 s, see
// docs/experiments/2026-09-30-build-speed.md). The pinned version and the test
// groups for state shared across processes live in .config/nextest.toml.
const nextest = spawnSync("cargo", ["nextest", "--version"], { env: gateEnv, encoding: "utf8" });
if (nextest.status !== 0) {
  console.error("cargo-nextest is required: cargo install cargo-nextest --locked");
  process.exit(1);
}

const workspace = process.argv[2] === "--workspace";
if (workspace) {
  // One compilation of each crate serves lib, bin and `it` tests. nextest does
  // not run doc-tests; the workspace has none, so lib targets set
  // `doctest = false` instead of paying for an empty pass here (KOS-291).
  run(cortex, ["nextest", "run", "--workspace", ...GATE_FEATURES]);
} else {
  run(cortex, [
    "nextest",
    "run",
    "-p",
    "engine",
    "--lib",
    ...GATE_FEATURES,
    ...process.argv.slice(2),
  ]);
}
