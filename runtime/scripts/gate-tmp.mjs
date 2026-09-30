import { mkdirSync, readdirSync, rmSync } from "node:fs";
import { join, resolve, sep } from "node:path";

// KOS-270: engine/ark-core tests must never write to the user's %TEMP%.
// `openGateTmp` creates a fresh per-run scratch directory inside the cargo
// target dir; the gate injects it as TMP/TEMP/TMPDIR for every cargo
// invocation so tempfile::tempdir() and friends land there. After the test
// processes have exited — when every leaked handle is closed — the gate
// sweeps the directory. Any leftover entry is a leaked fixture and fails the
// gate, so a leak is a red gate instead of silent accumulation.

export function openGateTmp(targetDir, runId) {
  const dir = resolve(targetDir, "gate-tmp", `run-${runId}`);
  mkdirSync(dir, { recursive: true });
  return dir;
}

// Removes `dir` and returns its leftover entries as `{ name, contents }`
// (empty list when the tests cleaned up after themselves). Contents are
// captured before removal so a leak stays attributable to its test.
// `dir` must live under `targetDir`: this function deletes recursively, and
// refusing anything outside the build tree is the only guard against a bad
// caller.
export function sweepGateTmp(targetDir, dir) {
  const root = resolve(targetDir, "gate-tmp") + sep;
  if (!resolve(dir).startsWith(root)) {
    throw new Error(`refusing to sweep ${dir}: not under ${root}`);
  }
  const leftover = readdirSync(dir).map((name) => {
    let contents;
    try {
      contents = readdirSync(join(dir, name));
    } catch {
      contents = [];
    }
    return { name, contents };
  });
  rmSync(dir, { recursive: true, force: true });
  return leftover;
}

// Sweeps and logs the result. Returns true when leftovers existed — the gate
// treats that as a leak and fails.
export function reportGateTmpSweep(targetDir, dir, log = console.log) {
  const leftover = sweepGateTmp(targetDir, dir);
  log(
    `gate tmp: swept ${leftover.length} leftover entr${leftover.length === 1 ? "y" : "ies"} under ${dir}`,
  );
  for (const { name, contents } of leftover.slice(0, 20)) {
    log(`gate tmp:   leftover ${name} (${contents.join(",") || "-"})`);
  }
  return leftover.length > 0;
}
